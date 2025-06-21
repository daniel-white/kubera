use crate::controllers::transformers::Backend;
use crate::ipc::IpcServices;
use crate::objects::{ObjectRef, ObjectState, Objects};
use anyhow::Result;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::{
    HTTPRoute, HTTPRouteRulesMatchesHeadersType, HTTPRouteRulesMatchesMethod,
    HTTPRouteRulesMatchesPathType, HTTPRouteRulesMatchesQueryParamsType,
};
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::api::{Patch, PatchParams, PostParams};
use kube::Client;
use kubera_api::constants::{
    CONFIGMAP_ROLE_GATEWAY_CONFIG, CONFIGMAP_ROLE_LABEL, MANAGED_BY_LABEL, MANAGED_BY_VALUE,
};
use kubera_core::config::gateway::serde::write_configuration;
use kubera_core::config::gateway::types::http::router::HttpMethodMatch;
use kubera_core::config::gateway::types::{GatewayConfiguration, GatewayConfigurationBuilder};
use kubera_core::ipc::{Event, GatewayEvent};
use kubera_core::net::Hostname;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::hash_map::{Entry, HashMap};
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::BufWriter;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::warn;

fn on_configuration_update(
    ipc_services: &IpcServices,
    configuration_hashes: &mut HashMap<ObjectRef, u64>,
    gateway_ref: &ObjectRef,
    configuration: &GatewayConfiguration,
) {
    let configuration_hash = {
        let mut hasher = DefaultHasher::new();
        configuration.hash(&mut hasher);
        hasher.finish()
    };
    let gateway_ref = gateway_ref.clone();
    match configuration_hashes.entry(gateway_ref.clone()) {
        Entry::Occupied(mut entry) => {
            if *entry.get() != configuration_hash {
                entry.insert(configuration_hash);
                send_configuration_update_event(ipc_services, &gateway_ref);
            }
        }
        Entry::Vacant(entry) => {
            entry.insert(configuration_hash);
            send_configuration_update_event(ipc_services, &gateway_ref);
        }
    }
}

fn send_configuration_update_event(ipc_services: &IpcServices, gateway_ref: &ObjectRef) {
    warn!(
        "Sending configuration update event for gateway: {}",
        gateway_ref
    );
    ipc_services
        .events()
        .send(Event::Gateway(GatewayEvent::ConfigurationUpdate {
            name: gateway_ref.name().to_string(),
            namespace: gateway_ref
                .namespace()
                .clone()
                .unwrap_or("default".to_string()),
        }));
}

pub fn generate_gateway_configuration(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    http_routes: &Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    backends: &Receiver<BTreeMap<ObjectRef, Backend>>,
    ipc_services: Arc<IpcServices>,
) -> Receiver<HashMap<ObjectRef, GatewayConfiguration>> {
    let (tx, rx) = channel(HashMap::default());

    let mut gateways = gateways.clone();
    let mut http_routes = http_routes.clone();
    let mut backends = backends.clone();

    let mut configuration_hashes = HashMap::default();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let current_backends = backends.current();
            let configs: HashMap<_, _> = current_gateways
                .iter()
                .map(|(gateway_ref, gateway_uid, gateway)| {
                    let mut gateway_configuration = GatewayConfigurationBuilder::new();

                    match gateway {
                        ObjectState::Active(gateway) => {
                            for listener in &gateway.spec.listeners {
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
                                .get(&gateway_ref)
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
                                            r.add_rule(
                                                format!(
                                                    "{}:{}:{}",
                                                    gateway_uid,
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
                                                                            b.add_endpoint(address, |e| {
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
                        }
                        ObjectState::Deleted(_) => {}
                    }

                    let gateway_configuration = gateway_configuration.build();
                    on_configuration_update(
                        ipc_services.as_ref(),
                        &mut configuration_hashes,
                        &gateway_ref,
                        &gateway_configuration,
                    );
                    (gateway_ref.clone(), gateway_configuration)
                })
                .collect();

            tx.replace(configs);

            select_continue!(gateways.changed(), http_routes.changed(), backends.changed());
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
        Some(hostname) if hostname.is_empty() => None,
        Some(hostname) if hostname.starts_with('*') => {
            let hostname = hostname.trim_start_matches('*');
            Some(HostnameMatchType::Suffix(Hostname::new(hostname)))
        }
        Some(hostname) => Some(HostnameMatchType::Exact(Hostname::new(hostname))),
        None => None,
    }
}

pub fn sync_gateway_configuration(
    join_set: &mut JoinSet<()>,
    client: &Client,
    config_maps: &Receiver<Objects<ConfigMap>>,
    gateway_configurations: &Receiver<HashMap<ObjectRef, GatewayConfiguration>>,
) {
    let mut config_maps = config_maps.clone();
    let mut gateway_configuration = gateway_configurations.clone();
    let client = client.clone();

    join_set.spawn(async move {
        loop {
            let current_config_maps = config_maps.current();
            let current_gateway_configs = gateway_configuration.current();
            for (gateway_ref, gateway_config) in current_gateway_configs.iter() {
                if let Ok((config_map_ref, config_map)) =
                    map_gateway_configuration_to_config_map(gateway_ref, gateway_config)
                {
                    let config_maps_api = kube::Api::<ConfigMap>::namespaced(
                        client.clone(),
                        config_map_ref.namespace().clone().as_deref().unwrap(),
                    );

                    match current_config_maps.as_ref().get_by_ref(&config_map_ref) {
                        Some(ObjectState::Active(_)) => {
                            let _ = config_maps_api
                                .patch(
                                    config_map_ref.name(),
                                    &PatchParams::default(),
                                    &Patch::Strategic(config_map),
                                )
                                .await
                                .inspect_err(|e| {
                                    warn!(
                                        "Failed to update ConfigMap {}: {}",
                                        config_map_ref.name(),
                                        e
                                    );
                                });
                        }
                        _ => {
                            let _ = config_maps_api
                                .create(&PostParams::default(), &config_map)
                                .await
                                .inspect_err(|e| {
                                    warn!(
                                        "Failed to create ConfigMap {}: {}",
                                        config_map_ref.name(),
                                        e
                                    );
                                });
                        }
                    }
                }
            }
            select_continue!(config_maps.changed(), gateway_configuration.changed());
        }
    });
}

fn map_gateway_configuration_to_config_map(
    gateway_ref: &ObjectRef,
    gateway_config: &GatewayConfiguration,
) -> Result<(ObjectRef, ConfigMap)> {
    let config_map_name = format!("{}-gateway-config", gateway_ref.name());
    let config_map_ref = ObjectRef::new_builder()
        .of_kind::<ConfigMap>()
        .namespace(gateway_ref.namespace().clone())
        .name(&config_map_name)
        .build()
        .expect("Failed to build ConfigMap reference");

    let mut buf = BufWriter::new(Vec::new());
    write_configuration(gateway_config, &mut buf)?;
    let file_content = String::from_utf8(buf.into_inner()?)?;

    let config_map = ConfigMap {
        metadata: kube::api::ObjectMeta {
            name: Some(config_map_name),
            namespace: gateway_ref.namespace().clone(),
            labels: Some(BTreeMap::from([
                (MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string()),
                (
                    CONFIGMAP_ROLE_LABEL.to_string(),
                    CONFIGMAP_ROLE_GATEWAY_CONFIG.to_string(),
                ),
            ])),
            ..Default::default()
        },
        data: Some(BTreeMap::from([("config.yaml".to_string(), file_content)])),
        ..Default::default()
    };

    Ok((config_map_ref, config_map))
}
