use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::{Backend, GatewayInstanceConfiguration};
use crate::ipc::IpcServices;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use derive_builder::Builder;
use gateway_api::apis::standard::httproutes::{
    HTTPRoute, HTTPRouteRulesMatchesHeadersType, HTTPRouteRulesMatchesMethod,
    HTTPRouteRulesMatchesPathType, HTTPRouteRulesMatchesQueryParamsType,
};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::runtime::watcher::Config;
use kubera_api::v1alpha1::{ClientAddresses, ClientAddressesSource, ProxyIpAddressHeaders};
use kubera_core::config::gateway::types::http::router::HttpMethodMatch;
use kubera_core::config::gateway::types::net::ProxyHeaders;
use kubera_core::config::gateway::types::{GatewayConfiguration, GatewayConfigurationBuilder};
use kubera_core::net::Hostname;
use kubera_core::sync::signal;
use kubera_core::sync::signal::Receiver;
use kubera_core::{continue_after, continue_on};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::select;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

const TEMPLATE: &str = include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

#[derive(Clone, Builder, Debug, Gtmpl)]
#[builder(setter(into))]
struct TemplateValues {
    gateway_name: String,
    config_yaml: String,
}

pub fn sync_gateway_configmaps(
    options: Arc<Options>,
    join_set: &mut JoinSet<()>,
    kube_client: &Receiver<Option<KubeClientCell>>,
    ipc_services: Arc<IpcServices>,
    instance_role: &Receiver<InstanceRole>,
    primary_instance_ip_addr: &Receiver<Option<IpAddr>>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    http_routes: &Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    backends: &Receiver<HashMap<ObjectRef, Backend>>,
) {
    let (tx, current_refs) = sync_objects!(
        options,
        join_set,
        ConfigMap,
        kube_client,
        instance_role,
        TemplateValues,
        TEMPLATE
    );

    generate_gateway_configmaps(
        options,
        join_set,
        tx,
        ipc_services,
        current_refs,
        primary_instance_ip_addr,
        gateway_instances,
        http_routes,
        backends,
    );
}

fn generate_gateway_configmaps(
    options: Arc<Options>,
    join_set: &mut JoinSet<()>,
    tx: Sender<SyncObjectAction<TemplateValues, ConfigMap>>,
    ipc_services: Arc<IpcServices>,
    current_refs_rx: Receiver<Option<HashSet<ObjectRef>>>,
    primary_instance_ip_addr: &Receiver<Option<IpAddr>>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    http_routes: &Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    backends: &Receiver<HashMap<ObjectRef, Backend>>,
) {
    let ipc_services = ipc_services.clone();
    let intended_rx = generate_gateway_configurations(
        join_set,
        ipc_services.clone(),
        primary_instance_ip_addr,
        gateway_instances,
        http_routes,
        backends,
    );

    join_set.spawn(async move {
        loop {
            info!("Reconciling Gateway ConfigMaps");
            let intended = intended_rx.current().clone();
            let intended: Vec<_> = intended
                .iter()
                .map(|(gateway_ref, config)| {
                    let configmap_ref = ObjectRef::new_builder()
                        .of_kind::<ConfigMap>()
                        .namespace(gateway_ref.namespace().clone())
                        .name(format!("{}-config", gateway_ref.name()))
                        .build()
                        .expect("Failed to build ObjectRef for ConfigMap");

                    let config_yaml = serde_yaml::to_string(config)
                        .expect("Failed to serialize GatewayConfiguration to YAML");
                    let template_values = TemplateValuesBuilder::default()
                        .gateway_name(gateway_ref.name())
                        .config_yaml(config_yaml)
                        .build()
                        .expect("Failed to build TemplateValues");

                    (configmap_ref, gateway_ref, template_values, config)
                })
                .collect();

            if let Some(current_refs) = current_refs_rx.current().as_ref() {
                let intended_refs: HashSet<_> = intended
                    .iter()
                    .map(|(ref_, _, _, _)| ref_.clone())
                    .collect();

                let deleted_refs = current_refs.difference(&intended_refs);

                for deleted_ref in deleted_refs {
                    let ipc_services = ipc_services.clone();
                    let _ = tx
                        .send(SyncObjectAction::Delete(deleted_ref.clone()))
                        .inspect(move |_| {
                            ipc_services.remove_gateway_configuration(&deleted_ref);
                        })
                        .inspect_err(|err| {
                            error!("Failed to send delete action: {}", err);
                        });
                }
            }

            for (service_ref, gateway_ref, template_values, config) in intended.into_iter() {
                let ipc_services = ipc_services.clone();
                let _ = tx
                    .send(SyncObjectAction::Upsert(
                        service_ref,
                        gateway_ref.clone(),
                        template_values,
                        None,
                    ))
                    .inspect(move |_| {
                        ipc_services.insert_gateway_configuration(gateway_ref.clone(), config);
                    })
                    .inspect_err(|err| {
                        error!("Failed to send upsert action: {}", err);
                    });
            }

            continue_after!(
                options.auto_cycle_duration(),
                intended_rx.changed(),
                current_refs_rx.changed()
            );
        }
    });
}

fn generate_gateway_configurations(
    join_set: &mut JoinSet<()>,
    ipc_services: Arc<IpcServices>,
    primary_instance_ip_addr: &Receiver<Option<IpAddr>>,
    gateway_instances: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    http_routes: &Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    backends: &Receiver<HashMap<ObjectRef, Backend>>,
) -> Receiver<HashMap<ObjectRef, GatewayConfiguration>> {
    let (tx, rx) = signal::channel(HashMap::default());

    let ipc_services = ipc_services.clone();
    let primary_instance_ip_addr = primary_instance_ip_addr.clone();
    let gateway_instances = gateway_instances.clone();
    let http_routes = http_routes.clone();
    let backends = backends.clone();

    join_set.spawn(async move {
        loop {
            let current_instances = gateway_instances.current();
            let current_backends = backends.current();
            let configs: HashMap<_, _> = current_instances
                .iter()
                .map(|(gateway_ref, instance)| {
                    info!("Generating configuration for gateway: {}", gateway_ref);
                    let mut gateway_configuration = GatewayConfigurationBuilder::new();

                    if let Some(ip_addr) = primary_instance_ip_addr.current().as_ref() {
                        gateway_configuration.with_ipc(|cp| {
                            cp.with_endpoint(ip_addr, &ipc_services.port());
                        });
                    }

                    gateway_configuration.with_client_addrs(|c| {
                        if let Some(client_addresses) = instance.configuration().client_addresses.as_ref() {
                            warn!("Configuring client addresses for gateway: {:?}", client_addresses);
                            match client_addresses.source {
                                ClientAddressesSource::None => {
                                    // No strategy, use default behavior
                                }
                                ClientAddressesSource::Header => {
                                    c.trust_header(client_addresses.header.clone().expect("Header must be set when using source: Header"));
                                }
                                ClientAddressesSource::Proxies => {
                                    c.trust_proxies(|p| {
                                        let proxies = client_addresses.proxies.as_ref().expect("Proxies must be set when using source: Proxies");
                                        if proxies.trust_local_ranges {
                                            p.trust_local_ranges();
                                        }
                                        for trusted_ip in &proxies.trusted_ips {
                                            p.add_trusted_ip(*trusted_ip);
                                        }
                                        for trusted_range in &proxies.trusted_ranges {
                                            p.add_trusted_range(*trusted_range);
                                        }
                                        for trusted_header in &proxies.trusted_headers {
                                            match trusted_header {
                                                ProxyIpAddressHeaders::Forwarded => p.add_trusted_header(ProxyHeaders::Forwarded),
                                                ProxyIpAddressHeaders::XForwardedFor => p.add_trusted_header(ProxyHeaders::XForwardedFor),
                                                ProxyIpAddressHeaders::XForwardedHost => p.add_trusted_header(ProxyHeaders::XForwardedHost),
                                                ProxyIpAddressHeaders::XForwardedProto => p.add_trusted_header(ProxyHeaders::XForwardedProto),
                                                ProxyIpAddressHeaders::XForwardedBy => p.add_trusted_header(ProxyHeaders::XForwardedBy),
                                            };
                                        }
                                    });
                                }
                            }
                        }
                    });

                    for listener in &instance.gateway().spec.listeners {
                        gateway_configuration.add_listener(|l| {
                            l.with_name(&listener.name)
                                .with_port(listener.port as u16)
                                .with_protocol(&listener.protocol);

                            match map_hostname_match_to_type(&listener.hostname) {
                                Some(HostnameMatchType::Exact(hostname)) => {
                                    l.with_exact_hostname(hostname);
                                }
                                Some(HostnameMatchType::Suffix(hostname)) => {
                                    l.with_hostname_suffix(hostname);
                                }
                                None => {}
                            };
                        });
                    }

                    for http_route in http_routes
                        .current()
                        .get(gateway_ref)
                        .unwrap_or(&vec![])
                        .iter()
                    {
                        gateway_configuration.add_http_route(|r| {
                            if let Some(hostnames) = &http_route.spec.hostnames {
                                for hostname in hostnames {
                                    match map_hostname_match_to_type(&Some(hostname)) {
                                        Some(HostnameMatchType::Exact(hostname)) => {
                                            r.add_exact_host_header(hostname);
                                        }
                                        Some(HostnameMatchType::Suffix(hostname)) => {
                                            r.add_host_header_with_suffix(hostname);
                                        }
                                        None => {}
                                    }
                                }
                            }

                            if let Some(rules) = &http_route.spec.rules {
                                for (index, rule) in rules.iter().enumerate() {
                                    let gateway = instance.gateway().as_ref();
                                    r.add_rule(
                                        format!(
                                            "{}:{}:{}",
                                            gateway.metadata.uid.clone().unwrap(),
                                            http_route.metadata.uid.clone().unwrap(),
                                            index
                                        ),
                                        |r| {
                                            if let Some(matches) = &rule.matches {
                                                for m in matches {
                                                    r.add_match(|config_m| {
                                                        if let Some(path) = &m.path {
                                                            match path.r#type {
                                                                Some(HTTPRouteRulesMatchesPathType::Exact) => {
                                                                    config_m.with_exact_path(path.value.clone().unwrap());
                                                                }
                                                                Some(HTTPRouteRulesMatchesPathType::PathPrefix) => {
                                                                    config_m.with_path_prefix(path.value.clone().unwrap());
                                                                }
                                                                Some(HTTPRouteRulesMatchesPathType::RegularExpression) => {
                                                                    config_m.with_path_matching(path.value.clone().unwrap());
                                                                }
                                                                None => {}
                                                            }
                                                        }

                                                        if let Some(method) = &m.method {
                                                            let method = match method {
                                                                HTTPRouteRulesMatchesMethod::Get => HttpMethodMatch::Get,
                                                                HTTPRouteRulesMatchesMethod::Head => HttpMethodMatch::Head,
                                                                HTTPRouteRulesMatchesMethod::Post => HttpMethodMatch::Post,
                                                                HTTPRouteRulesMatchesMethod::Put => HttpMethodMatch::Put,
                                                                HTTPRouteRulesMatchesMethod::Delete => HttpMethodMatch::Delete,
                                                                HTTPRouteRulesMatchesMethod::Connect => HttpMethodMatch::Connect,
                                                                HTTPRouteRulesMatchesMethod::Options => HttpMethodMatch::Options,
                                                                HTTPRouteRulesMatchesMethod::Trace => HttpMethodMatch::Trace,
                                                                HTTPRouteRulesMatchesMethod::Patch => HttpMethodMatch::Patch,
                                                            };

                                                            config_m.with_method(method);
                                                        }

                                                        if let Some(headers) = &m.headers {
                                                            for header in headers {
                                                                match header.r#type.clone().unwrap_or(HTTPRouteRulesMatchesHeadersType::Exact) {
                                                                    HTTPRouteRulesMatchesHeadersType::Exact => {
                                                                        config_m.add_exact_header(
                                                                            header.name.as_str(),
                                                                            header.value.as_str(),
                                                                        );
                                                                    }
                                                                    HTTPRouteRulesMatchesHeadersType::RegularExpression => {
                                                                        config_m.add_header_matching(
                                                                            header.name.as_str(),
                                                                            header.value.as_str(),
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        if let Some(query_params) = &m.query_params {
                                                            for query_param in query_params {
                                                                match query_param.r#type.clone().unwrap_or(HTTPRouteRulesMatchesQueryParamsType::Exact) {
                                                                    HTTPRouteRulesMatchesQueryParamsType::Exact => {
                                                                        config_m.add_exact_query_param(
                                                                            query_param.name.as_str(),
                                                                            query_param.value.as_str(),
                                                                        );
                                                                    }
                                                                    HTTPRouteRulesMatchesQueryParamsType::RegularExpression => {
                                                                        config_m.add_query_param_matching(
                                                                            query_param.name.as_str(),
                                                                            query_param.value.as_str(),
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                            }

                                            if let Some(backend_refs) = &rule.backend_refs {
                                                for backend_ref in backend_refs {
                                                    let backend_ref = ObjectRef::new_builder()
                                                        .of_kind::<Service>()
                                                        .namespace(backend_ref.namespace.clone().or_else(|| http_route.metadata.namespace.clone()))
                                                        .name(&backend_ref.name)
                                                        .build()
                                                        .expect("Failed to build Backend reference");

                                                    if let Some(backend) = current_backends.get(&backend_ref) {
                                                        r.add_backend(|b| {
                                                            let object_ref = backend.object_ref();
                                                            b.named(object_ref.name())
                                                                .with_namespace(object_ref.namespace().as_ref())
                                                                .with_port(backend.port().map(|p| p as u16))
                                                                .with_weight(*backend.weight());

                                                            for endpoint in backend.endpoints() {
                                                                for address in endpoint.addresses() {
                                                                    b.add_endpoint(*address, |e| {
                                                                        let zone_ref = endpoint.location();
                                                                        if let Some(node) = zone_ref.node() {
                                                                            e.with_node(node);
                                                                        }
                                                                        if let Some(zone) = zone_ref.zone() {
                                                                            e.with_zone(zone);
                                                                        }
                                                                    });
                                                                }
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                        },
                                    );
                                }
                            }
                        });
                    }


                    let gateway_configuration = gateway_configuration.build();
                    (gateway_ref.clone(), gateway_configuration)
                })
                .collect();

            tx.replace(configs);

            continue_on!(
                primary_instance_ip_addr.changed(),
                gateway_instances.changed(),
                http_routes.changed(),
                backends.changed()
            );
        }
    });

    rx
}

enum HostnameMatchType {
    Exact(Hostname),
    Suffix(Hostname),
}

fn map_hostname_match_to_type<S: AsRef<str>>(hostname: &Option<S>) -> Option<HostnameMatchType> {
    match hostname.as_ref().map(|hostname| hostname.as_ref()) {
        Some("") => None,
        Some(hostname) if hostname.starts_with('*') => {
            let hostname = hostname.trim_start_matches('*');
            Some(HostnameMatchType::Suffix(Hostname::new(hostname)))
        }
        Some(hostname) => Some(HostnameMatchType::Exact(Hostname::new(hostname))),
        None => None,
    }
}
