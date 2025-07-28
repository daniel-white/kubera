use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::{Backend, GatewayInstanceConfiguration};
use crate::ipc::IpcServices;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use gateway_api::apis::standard::httproutes::{
    HTTPRoute, HTTPRouteRulesMatchesHeadersType, HTTPRouteRulesMatchesMethod,
    HTTPRouteRulesMatchesPathType, HTTPRouteRulesMatchesQueryParamsType,
};
use gateway_api::gateways::Gateway;
use gateway_api::httproutes::HTTPRouteRulesMatches;
use getset::CloneGetters;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::runtime::watcher::Config;
use kubera_api::v1alpha1::{ClientAddressesSource, ProxyIpAddressHeaders};
use kubera_core::config::gateway::types::http::router::{
    HttpMethodMatch, HttpRouteBuilder, HttpRouteRuleBuilder, HttpRouteRuleMatchesBuilder,
};
use kubera_core::config::gateway::types::net::ProxyHeaders;
use kubera_core::config::gateway::types::{GatewayConfiguration, GatewayConfigurationBuilder};
use kubera_core::net::{Hostname, Port};
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use kubera_core::{continue_after, continue_on};
use kubera_macros::await_ready;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::select;
use tokio::sync::broadcast::Sender;
use tracing::{debug, error, info, warn};
use typed_builder::TypedBuilder;

const TEMPLATE: &str = include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into))]
    config_yaml: String,
}

#[derive(TypedBuilder, CloneGetters, Clone)]
pub struct SyncGatewayConfigmapsParams {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,
    #[getset(get_clone = "pub")]
    kube_client_rx: Receiver<KubeClientCell>,
    #[getset(get_clone = "pub")]
    ipc_services: Arc<IpcServices>,
    #[getset(get_clone = "pub")]
    instance_role_rx: Receiver<InstanceRole>,
    #[getset(get_clone = "pub")]
    primary_instance_ip_addr_rx: Receiver<IpAddr>,
    #[getset(get_clone = "pub")]
    gateway_instances_rx: Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    #[getset(get_clone = "pub")]
    http_routes_rx: Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    #[getset(get_clone = "pub")]
    backends_rx: Receiver<HashMap<ObjectRef, Backend>>,
}

pub fn sync_gateway_configmaps(task_builder: &TaskBuilder, params: SyncGatewayConfigmapsParams) {
    let options = params.options();
    let kube_client_rx = params.kube_client_rx();
    let instance_role_rx = params.instance_role_rx();

    let (tx, current_refs_rx) = sync_objects!(
        options,
        task_builder,
        ConfigMap,
        kube_client_rx,
        instance_role_rx,
        TemplateValues,
        TEMPLATE
    );

    let params = GenerateGatewayConfigmapsParams::builder()
        .options(params.options())
        .sync_tx(tx)
        .ipc_services(params.ipc_services())
        .current_refs_rx(current_refs_rx)
        .primary_instance_ip_addr_rx(params.primary_instance_ip_addr_rx())
        .gateway_instances_rx(params.gateway_instances_rx())
        .http_routes_rx(params.http_routes_rx())
        .backends_rx(params.backends_rx())
        .build();

    generate_gateway_configmaps(task_builder, params);
}

#[derive(TypedBuilder, CloneGetters, Clone)]
struct GenerateGatewayConfigmapsParams {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,
    #[getset(get_clone = "pub")]
    sync_tx: Sender<SyncObjectAction<TemplateValues, ConfigMap>>,
    #[getset(get_clone = "pub")]
    ipc_services: Arc<IpcServices>,
    #[getset(get_clone = "pub")]
    current_refs_rx: Receiver<HashSet<ObjectRef>>,
    #[getset(get_clone = "pub")]
    primary_instance_ip_addr_rx: Receiver<IpAddr>,
    #[getset(get_clone = "pub")]
    gateway_instances_rx: Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    #[getset(get_clone = "pub")]
    http_routes_rx: Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    #[getset(get_clone = "pub")]
    backends_rx: Receiver<HashMap<ObjectRef, Backend>>,
}

fn generate_gateway_configmaps(
    task_builder: &TaskBuilder,
    params: GenerateGatewayConfigmapsParams,
) {
    let gateway_configurations_rx = generate_gateway_configurations(
        task_builder,
        params.ipc_services(),
        params.primary_instance_ip_addr_rx(),
        params.gateway_instances_rx(),
        params.http_routes_rx(),
        params.backends_rx(),
    );

    task_builder
        .new_task(stringify!(sync_gateway_configmaps))
        .spawn(async move {
            let current_refs_rx = params.current_refs_rx();
            loop {
                await_ready!(gateway_configurations_rx, current_refs_rx)
                    .and_then(async |gateway_configurations, current_configmap_refs| {
                        info!("Reconciling Gateway ConfigMaps");
                        let desired_gateway_configurations = expand(&gateway_configurations);

                        let desire_configmap_refs: HashSet<_> = desired_gateway_configurations
                            .iter()
                            .map(|state| state.configmap_ref.clone())
                            .collect();

                        let deleted_refs =
                            current_configmap_refs.difference(&desire_configmap_refs);
                        for deleted_ref in deleted_refs {
                            let _ = params
                                .sync_tx
                                .send(SyncObjectAction::Delete(deleted_ref.clone()))
                                .inspect(|_| {
                                    params
                                        .ipc_services()
                                        .remove_gateway_configuration(deleted_ref);
                                })
                                .inspect_err(|err| {
                                    error!("Failed to send delete action: {}", err);
                                });
                        }

                        'send_and_insert: for gateway_state in desired_gateway_configurations {
                            let Some((template_values, config)) = gateway_state.values else {
                                continue 'send_and_insert;
                            };

                            if let Err(err) = params.sync_tx.send(SyncObjectAction::Upsert(
                                gateway_state.configmap_ref,
                                gateway_state.gateway_ref.clone(),
                                template_values,
                                None,
                            )) {
                                warn!("Failed to send upsert action: {}", err);
                                continue 'send_and_insert;
                            }

                            if let Err(err) = params
                                .ipc_services()
                                .try_insert_gateway_configuration(gateway_state.gateway_ref, config)
                            {
                                warn!("Failed to insert gateway configuration: {}", err);
                                continue 'send_and_insert;
                            }
                        }
                    })
                    .run()
                    .await;

                continue_after!(
                    params.options.auto_cycle_duration(),
                    gateway_configurations_rx.changed(),
                    params.current_refs_rx.changed()
                );
            }
        });
}

#[derive(Clone, Debug, TypedBuilder)]
struct GatewayState {
    gateway_ref: ObjectRef,
    configmap_ref: ObjectRef,
    values: Option<(TemplateValues, GatewayConfiguration)>,
}

fn expand(configurations: &HashMap<ObjectRef, Option<GatewayConfiguration>>) -> Vec<GatewayState> {
    configurations
        .iter()
        .map(|(gateway_ref, config)| {
            let configmap_ref = ObjectRef::of_kind::<ConfigMap>()
                .namespace(gateway_ref.namespace().clone())
                .name(format!("{}-config", gateway_ref.name()))
                .build();

            let state = GatewayState::builder()
                .gateway_ref(gateway_ref.clone())
                .configmap_ref(configmap_ref);

            let state = if let Some(config) = config {
                let config_yaml = serde_yaml::to_string(config)
                    .expect("Failed to serialize GatewayConfiguration to YAML");
                let template_values = TemplateValues::builder()
                    .gateway_name(gateway_ref.name())
                    .config_yaml(config_yaml)
                    .build();

                state.values(Some((template_values, config.clone())))
            } else {
                warn!("No configuration found for gateway: {}", gateway_ref);
                state.values(None)
            };

            state.build()
        })
        .collect::<Vec<_>>()
}

fn generate_gateway_configurations(
    task_builder: &TaskBuilder,
    ipc_services: Arc<IpcServices>,
    primary_instance_ip_addr_rx: Receiver<IpAddr>,
    gateway_instances_rx: Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    http_routes_rx: Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    backends_rx: Receiver<HashMap<ObjectRef, Backend>>,
) -> Receiver<HashMap<ObjectRef, Option<GatewayConfiguration>>> {
    let (tx, rx) = signal();

    task_builder
        .new_task(stringify!(generate_gateway_configurations))
        .spawn(async move {
        loop {
            await_ready!(
                primary_instance_ip_addr_rx,
                gateway_instances_rx,
                backends_rx,
                http_routes_rx,
            ).and_then(async |primary_instance_ip_addr, gateway_instances, backends, http_routes| {
                let configs: HashMap<_, _> = gateway_instances
                    .iter()
                    .map(|(gateway_ref, instance)| {
                        info!("Generating configuration for gateway: {}", gateway_ref);
                        let mut gateway_configuration = GatewayConfigurationBuilder::default();

                        set_ipc(
                            &mut gateway_configuration,
                            &ipc_services,
                            primary_instance_ip_addr,
                        );
                        set_client_addrs_strategy(&mut gateway_configuration, instance);
                        add_listeners(&mut gateway_configuration, instance);

                        let http_routes = http_routes
                            .get(gateway_ref)
                            .cloned()
                            .unwrap_or_default();
                        for http_route in http_routes {
                            gateway_configuration.add_http_route(|r| {
                                add_host_header_matches_for_route(&http_route, r);

                                if let Some(rules) = &http_route.spec.rules {
                                    for (index, rule) in rules.iter().enumerate() {
                                        let Some(rule_id) = format_rule_id(instance.gateway().as_ref(), &http_route, index) else {
                                            warn!("Failed to format rule ID for HTTPRoute {:?} at index {index} in gateway {gateway_ref}", http_route.metadata.name);
                                            continue;
                                        };
                                        r.add_rule(&rule_id,
                                                   |target| {
                                                       for source in rule.matches.iter().flatten() {
                                                           target.add_match(|target| {
                                                               add_method_matches(source, target);
                                                               add_path_matches(source, target);
                                                               add_header_matches(source, target);
                                                               add_query_params_matches(source, target);
                                                           });
                                                       }


                                                       for backend_ref in rule.backend_refs.iter().flatten() {
                                                           let source_ref = ObjectRef::of_kind::<Service>()
                                                               .namespace(
                                                                   backend_ref.namespace.clone().or_else(
                                                                       || {
                                                                           http_route
                                                                               .metadata
                                                                               .namespace
                                                                               .clone()
                                                                       },
                                                                   ),
                                                               )
                                                               .name(&backend_ref.name)
                                                               .build();

                                                           match backends.get(&source_ref) {
                                                               Some(source) => {
                                                                   add_backend(source, target);
                                                               }
                                                               None => {
                                                                   warn!(
                                                                       "Backend reference {} not found for HTTPRoute {:?} at rule index {}",
                                                                       backend_ref.name, http_route.metadata.name, index
                                                                   );
                                                               }
                                                           }
                                                       }
                                                   },
                                        );
                                    }
                                }
                            });
                        }

                        match gateway_configuration.build() {
                            Ok(gateway_configuration) => (gateway_ref.clone(), Some(gateway_configuration)),
                            Err(err) => {
                                error!("Failed to build GatewayConfiguration for {}: {}", gateway_ref, err);
                                (gateway_ref.clone(), None)
                            }
                        }
                    })
                    .collect();

                tx.set(configs).await;
            }).run().await;

            continue_on!(
                primary_instance_ip_addr_rx.changed(),
                gateway_instances_rx.changed(),
                http_routes_rx.changed(),
                backends_rx.changed()
            );
        }
    });

    rx
}

fn format_rule_id(gateway: &Gateway, route: &HTTPRoute, idx: usize) -> Option<String> {
    let gateway_uid = gateway.metadata.uid.as_ref()?;
    let route_uid = route.metadata.uid.as_ref()?;

    Some(format!("{gateway_uid}:{route_uid}:{idx}"))
}

fn add_backend(backend: &Backend, target: &mut HttpRouteRuleBuilder) {
    target.add_backend(|target| {
        let object_ref = backend.object_ref();
        target
            .named(object_ref.name())
            .with_namespace(object_ref.namespace().as_ref())
            .with_port(backend.port())
            .with_weight(backend.weight());

        for endpoint in backend.endpoints() {
            for address in endpoint.addresses().iter().copied() {
                target.add_endpoint(address, |target| {
                    let zone_ref = endpoint.location();
                    if let Some(node) = zone_ref.node() {
                        target.with_node(node);
                    }
                    if let Some(zone) = zone_ref.zone() {
                        target.with_zone(zone);
                    }
                });
            }
        }
    });
}

fn add_query_params_matches(
    source: &HTTPRouteRulesMatches,
    target: &mut HttpRouteRuleMatchesBuilder,
) {
    for query_param in source.query_params.iter().flatten() {
        match query_param
            .r#type
            .as_ref()
            .unwrap_or(&HTTPRouteRulesMatchesQueryParamsType::Exact)
        {
            HTTPRouteRulesMatchesQueryParamsType::Exact => {
                target.add_exact_query_param(query_param.name.as_str(), query_param.value.as_str());
            }
            HTTPRouteRulesMatchesQueryParamsType::RegularExpression => {
                target.add_query_param_matching(
                    query_param.name.as_str(),
                    query_param.value.as_str(),
                );
            }
        }
    }
}

fn add_header_matches(source: &HTTPRouteRulesMatches, target: &mut HttpRouteRuleMatchesBuilder) {
    for header in source.headers.iter().flatten() {
        match header
            .r#type
            .as_ref()
            .unwrap_or(&HTTPRouteRulesMatchesHeadersType::Exact)
        {
            HTTPRouteRulesMatchesHeadersType::Exact => {
                target.add_exact_header(&header.name, &header.value);
            }
            HTTPRouteRulesMatchesHeadersType::RegularExpression => {
                target.add_header_matching(&header.name, &header.value);
            }
        }
    }
}

fn add_path_matches(source: &HTTPRouteRulesMatches, target: &mut HttpRouteRuleMatchesBuilder) {
    if let Some(path) = &source.path {
        match (path.r#type.as_ref(), path.value.as_ref()) {
            (Some(HTTPRouteRulesMatchesPathType::Exact), Some(value)) => {
                target.with_exact_path(value);
            }
            (Some(HTTPRouteRulesMatchesPathType::PathPrefix), Some(value)) => {
                target.with_path_prefix(value);
            }
            (Some(HTTPRouteRulesMatchesPathType::RegularExpression), Some(value)) => {
                target.with_path_matching(value);
            }
            _ => {
                warn!("Unsupported path match type or missing value: {:?}", path);
            }
        }
    } else {
        warn!("No path match specified in source: {:?}", source);
    }
}

fn add_method_matches(source: &HTTPRouteRulesMatches, target: &mut HttpRouteRuleMatchesBuilder) {
    if let Some(method) = &source.method {
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

        target.with_method(method);
    }
}

fn add_host_header_matches_for_route(source: &Arc<HTTPRoute>, target: &mut HttpRouteBuilder) {
    for hostname in source.spec.hostnames.iter().flatten() {
        match map_hostname_match_to_type(Some(hostname)) {
            Some(HostnameMatchType::Exact(hostname)) => {
                target.add_exact_host_header(hostname);
            }
            Some(HostnameMatchType::Suffix(hostname)) => {
                target.add_host_header_with_suffix(hostname);
            }
            None => {}
        }
    }
}

fn set_ipc(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    ipc_services: &IpcServices,
    primary_instance_ip_addr: IpAddr,
) {
    gateway_configuration.with_ipc(|cp| {
        cp.with_endpoint(primary_instance_ip_addr, ipc_services.port());
    });
}

fn add_listeners(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    instance: &GatewayInstanceConfiguration,
) {
    for listener in &instance.gateway().spec.listeners {
        gateway_configuration.add_listener(|l| {
            l.with_name(&listener.name)
                .with_port(Port::new(
                    u16::try_from(listener.port).expect("Port must be a valid u16"),
                ))
                .with_protocol(&listener.protocol);

            match map_hostname_match_to_type(listener.hostname.as_deref()) {
                Some(HostnameMatchType::Exact(hostname)) => {
                    l.with_exact_hostname(hostname);
                }
                Some(HostnameMatchType::Suffix(hostname)) => {
                    l.with_hostname_suffix(hostname);
                }
                None => {}
            }
        });
    }
}

fn set_client_addrs_strategy(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    instance: &GatewayInstanceConfiguration,
) {
    if let Some(client_addresses) = instance.configuration().client_addresses.as_ref() {
        gateway_configuration.with_client_addrs(|c| {
            warn!(
                "Configuring client addresses for gateway: {:?}",
                client_addresses
            );
            match client_addresses.source {
                ClientAddressesSource::None => {
                    // No strategy, use default behavior
                }
                ClientAddressesSource::Header => {
                    c.trust_header(
                        client_addresses
                            .header
                            .clone()
                            .expect("Header must be set when using source: Header"),
                    );
                }
                ClientAddressesSource::Proxies => {
                    c.trust_proxies(|p| {
                        let proxies = client_addresses
                            .proxies
                            .as_ref()
                            .expect("Proxies must be set when using source: Proxies");
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
                                ProxyIpAddressHeaders::Forwarded => {
                                    p.add_trusted_header(ProxyHeaders::Forwarded)
                                }
                                ProxyIpAddressHeaders::XForwardedFor => {
                                    p.add_trusted_header(ProxyHeaders::XForwardedFor)
                                }
                                ProxyIpAddressHeaders::XForwardedHost => {
                                    p.add_trusted_header(ProxyHeaders::XForwardedHost)
                                }
                                ProxyIpAddressHeaders::XForwardedProto => {
                                    p.add_trusted_header(ProxyHeaders::XForwardedProto)
                                }
                                ProxyIpAddressHeaders::XForwardedBy => {
                                    p.add_trusted_header(ProxyHeaders::XForwardedBy)
                                }
                            };
                        }
                    });
                }
            }
        });
    }
}

enum HostnameMatchType {
    Exact(Hostname),
    Suffix(Hostname),
}

fn map_hostname_match_to_type(hostname: Option<&str>) -> Option<HostnameMatchType> {
    match hostname {
        Some("") | None => None,
        Some(hostname) if hostname.starts_with('*') => {
            let hostname = hostname.trim_start_matches('*');
            Some(HostnameMatchType::Suffix(Hostname::new(hostname)))
        }
        Some(hostname) => Some(HostnameMatchType::Exact(Hostname::new(hostname))),
    }
}
