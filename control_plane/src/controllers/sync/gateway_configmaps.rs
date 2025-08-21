use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::{
    Backend, ExtensionFilterKind, ExtensionFilters, GatewayInstanceConfiguration,
};
use crate::ipc::IpcServices;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
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
use kube::ResourceExt;
use kube::runtime::watcher::Config;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info, warn};
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::{
    AccessControlFilter, AccessControlFilterEffect, ClientAddressesSource, ErrorResponseKind,
    ProxyIpAddressHeaders, StaticResponseFilter,
};
use vg_core::config::gateway::types::http::filters::{
    ExtAccessControlRef, ExtStaticResponseRef, HTTPHeader, HttpRouteFilter, HttpRouteFilterType,
    RequestHeaderModifier, ResponseHeaderModifier,
};
use vg_core::config::gateway::types::http::router::{
    HttpMethodMatch, HttpRouteBuilder, HttpRouteRuleBuilder, HttpRouteRuleMatchesBuilder,
};
use vg_core::config::gateway::types::net::{
    AccessControlFilter as ConfigAccessControlFilter,
    AccessControlFilterClientMatches as ConfigAccessControlFilterClientMatches,
    AccessControlFilterEffect as ConfigAccessControlEffect,
    ErrorResponseKind as ConfigErrorResponseKind, ErrorResponses as ConfigErrorResponses,
    ProblemDetailErrorResponse, ProxyHeaders, StaticResponse, StaticResponseBody,
};
use vg_core::config::gateway::types::{GatewayConfiguration, GatewayConfigurationBuilder};
use vg_core::net::{Hostname, Port};
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{continue_after, continue_on};
use vg_macros::await_ready;

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
    #[getset(get_clone = "pub")]
    extension_filters_rx: Receiver<HashMap<ObjectRef, ExtensionFilters>>,
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
        .extension_filters_rx(params.extension_filters_rx())
        .build();

    generate_gateway_configmaps(task_builder, params);
}

#[derive(TypedBuilder, CloneGetters, Clone)]
struct GenerateGatewayConfigmapsParams {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,
    #[getset(get_clone = "pub")]
    sync_tx: UnboundedSender<SyncObjectAction<TemplateValues, ConfigMap>>,
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
    #[getset(get_clone = "pub")]
    extension_filters_rx: Receiver<HashMap<ObjectRef, ExtensionFilters>>,
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
        params.extension_filters_rx(),
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
                let config_yaml = match serde_yaml::to_string(config) {
                    Ok(yaml) => yaml,
                    Err(err) => {
                        warn!(
                            "Failed to serialize configuration for gateway {}: {}",
                            gateway_ref, err
                        );
                        return state.values(None).build();
                    }
                };

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
    extension_filters_rx: Receiver<HashMap<ObjectRef, ExtensionFilters>>,
) -> Receiver<HashMap<ObjectRef, Option<GatewayConfiguration>>> {
    let (tx, rx) = signal("generated_gateway_configurations");

    task_builder
        .new_task(stringify!(generate_gateway_configurations))
        .spawn(async move {
            loop {
                await_ready!(
                    primary_instance_ip_addr_rx,
                    gateway_instances_rx,
                    http_routes_rx,
                    backends_rx,
                    extension_filters_rx
                )
                .and_then(
                    async |primary_instance_ip_addr,
                           gateway_instances,
                           http_routes,
                           backends,
                           extension_filters| {
                        let configs: HashMap<ObjectRef, Option<GatewayConfiguration>> =
                            gateway_instances
                                .iter()
                                .map(|(gateway_ref, gateway_instance)| {
                                    let mut gateway_configuration =
                                        GatewayConfigurationBuilder::default();
                                    let extension_filters = extension_filters.get(gateway_ref);

                                    set_ipc(
                                        &mut gateway_configuration,
                                        &ipc_services,
                                        primary_instance_ip_addr,
                                    );
                                    set_client_addrs_strategy(
                                        &mut gateway_configuration,
                                        gateway_instance,
                                    );
                                    set_error_responses_strategy(
                                        &mut gateway_configuration,
                                        gateway_instance,
                                    );
                                    if let Some(extension_filters) = extension_filters {
                                        apply_static_response_filters(
                                            &mut gateway_configuration,
                                            extension_filters,
                                        );
                                        apply_access_control_filters(
                                            &mut gateway_configuration,
                                            extension_filters,
                                        );
                                    }

                                    add_listeners(&mut gateway_configuration, gateway_instance);

                                    process_http_routes(
                                        gateway_ref,
                                        gateway_instance,
                                        &http_routes,
                                        &backends,
                                        &mut gateway_configuration,
                                    );

                                    match gateway_configuration.build() {
                                        Ok(gateway_configuration) => {
                                            (gateway_ref.clone(), Some(gateway_configuration))
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to build GatewayConfiguration for {}: {}",
                                                gateway_ref, err
                                            );
                                            (gateway_ref.clone(), None)
                                        }
                                    }
                                })
                                .collect();

                        tx.set(configs).await;
                    },
                )
                .run()
                .await;

                continue_on!(
                    primary_instance_ip_addr_rx.changed(),
                    gateway_instances_rx.changed(),
                    http_routes_rx.changed(),
                    backends_rx.changed()
                )
            }
        });

    rx
}

fn apply_access_control_filters(
    mut gateway_configuration: &mut GatewayConfigurationBuilder,
    extension_filters: &ExtensionFilters,
) {
    if !extension_filters.access_controls().is_empty() {
        let filters = extension_filters
            .access_controls()
            .iter()
            .map(|(ref_, _, filter)| {
                let effect = match filter.spec.effect {
                    AccessControlFilterEffect::Allow => ConfigAccessControlEffect::Allow,
                    AccessControlFilterEffect::Deny => ConfigAccessControlEffect::Deny,
                };

                let clients = ConfigAccessControlFilterClientMatches::builder()
                    .ip_ranges(filter.spec.clients.ip_ranges.clone())
                    .ips(filter.spec.clients.ips.clone())
                    .build();

                ConfigAccessControlFilter::builder()
                    .key(ref_.to_string())
                    .effect(effect)
                    .clients(clients)
                    .build()
            })
            .collect();

        gateway_configuration.with_access_control_filters(filters);
    }
}

fn apply_static_response_filters(
    builder: &mut GatewayConfigurationBuilder,
    extension_filters: &ExtensionFilters,
) {
    let static_responses: Vec<_> = extension_filters
        .static_responses()
        .iter()
        .filter_map(|(ref_, _, filter)| {
            let spec = &filter.spec;

            let version_key = filter.metadata.resource_version.as_ref()?;
            let uid = filter.uid()?;

            let builder = StaticResponse::builder()
                .key(ref_.to_string())
                .version_key(version_key)
                .status_code(spec.status_code);

            if let Some(body) = &spec.body {
                let body_result = StaticResponseBody::builder()
                    .content_type(body.content_type.clone())
                    .identifier(uid)
                    .build();

                Some(builder.body(body_result).build())
            } else {
                Some(builder.build())
            }
        })
        .collect();

    builder.with_static_responses(static_responses);
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

#[allow(clippy::too_many_lines)]
fn process_http_routes(
    gateway_ref: &ObjectRef,
    gateway_instance: &GatewayInstanceConfiguration,
    http_routes: &HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>,
    backends: &HashMap<ObjectRef, Backend>,
    gateway_configuration: &mut GatewayConfigurationBuilder,
) {
    // Find routes that reference this gateway
    for http_routes_for_ref in http_routes.values() {
        for http_route in http_routes_for_ref {
            // Check if this route references our gateway
            let references_this_gateway =
                http_route
                    .spec
                    .parent_refs
                    .as_ref()
                    .is_some_and(|parent_refs| {
                        parent_refs.iter().any(|parent_ref| {
                            parent_ref.name == *gateway_ref.name()
                                && parent_ref.namespace.as_ref().unwrap_or(
                                    &http_route.metadata.namespace.clone().unwrap_or_default(),
                                ) == gateway_ref.namespace().as_ref().unwrap_or(&String::new())
                        })
                    });

            if !references_this_gateway {
                continue;
            }

            gateway_configuration.add_http_route(|r| {
                add_host_header_matches_for_route(http_route, r);

                // Process rules - handle the Option<Vec<HTTPRouteRules>> properly
                if let Some(rules) = &http_route.spec.rules {
                    for (index, rule) in rules.iter().enumerate() {
                        let rule_id = format_rule_id(gateway_instance.gateway(), http_route, index)
                            .unwrap_or_else(|| format!("rule-{index}"));

                        r.add_rule(rule_id, |target| {
                            // Process filters from HTTPRoute rule
                            if let Some(filters) = &rule.filters {
                                for filter in filters {
                                    if let Some(request_header_modifier) = &filter.request_header_modifier {
                                        // Convert Gateway API RequestHeaderModifier to Vale Gateway RequestHeaderModifier
                                        let mut vg_modifier = RequestHeaderModifier::default();

                                        // Convert set headers
                                        if let Some(set_headers) = &request_header_modifier.set {
                                            vg_modifier.set = Some(set_headers.iter().map(|h| HTTPHeader {
                                                name: h.name.clone(),
                                                value: h.value.clone(),
                                            }).collect());
                                        }

                                        // Convert add headers
                                        if let Some(add_headers) = &request_header_modifier.add {
                                            vg_modifier.add = Some(add_headers.iter().map(|h| HTTPHeader {
                                                name: h.name.clone(),
                                                value: h.value.clone(),
                                            }).collect());
                                        }

                                        // Convert remove headers
                                        if let Some(remove_headers) = &request_header_modifier.remove {
                                            vg_modifier.remove = Some(remove_headers.clone());
                                        }

                                        let vg_filter = HttpRouteFilter {
                                            filter_type: HttpRouteFilterType::RequestHeaderModifier,
                                            request_header_modifier: Some(vg_modifier),
                                            response_header_modifier: None,
                                            request_mirror: None,
                                            request_redirect: None,
                                            url_rewrite: None,
                                            ext_static_response: None,
                                            ext_access_control: None,
                                        };

                                        target.add_filter(vg_filter);
                                    }
                                    if let Some(response_header_modifier) = &filter.response_header_modifier {
                                        // Convert Gateway API ResponseHeaderModifier to Vale Gateway ResponseHeaderModifier
                                        let mut vg_modifier = ResponseHeaderModifier::default();

                                        // Convert set headers
                                        if let Some(set_headers) = &response_header_modifier.set {
                                            vg_modifier.set = Some(set_headers.iter().map(|h| HTTPHeader {
                                                name: h.name.clone(),
                                                value: h.value.clone(),
                                            }).collect());
                                        }

                                        // Convert add headers
                                        if let Some(add_headers) = &response_header_modifier.add {
                                            vg_modifier.add = Some(add_headers.iter().map(|h| HTTPHeader {
                                                name: h.name.clone(),
                                                value: h.value.clone(),
                                            }).collect());
                                        }

                                        // Convert remove headers
                                        if let Some(remove_headers) = &response_header_modifier.remove {
                                            vg_modifier.remove = Some(remove_headers.clone());
                                        }

                                        // Add filter to Vale Gateway configuration
                                        let vg_filter = HttpRouteFilter {
                                            filter_type: HttpRouteFilterType::ResponseHeaderModifier,
                                            request_header_modifier: None,
                                            response_header_modifier: Some(vg_modifier),
                                            request_mirror: None,
                                            request_redirect: None,
                                            url_rewrite: None,
                                            ext_static_response: None,
                                            ext_access_control: None,
                                        };

                                        target.add_filter(vg_filter);
                                    }
                                    if let Some(request_redirect) = &filter.request_redirect {
                                        // Convert Gateway API RequestRedirect to Vale Gateway RequestRedirect
                                        use crate::controllers::filters::gateway_api_converter::convert_request_redirect;

                                        let vg_redirect = convert_request_redirect(request_redirect);
                                        let vg_filter = HttpRouteFilter {
                                            filter_type: HttpRouteFilterType::RequestRedirect,
                                            request_header_modifier: None,
                                            response_header_modifier: None,
                                            request_mirror: None,
                                            request_redirect: Some(vg_redirect),
                                            url_rewrite: None,
                                            ext_static_response: None,
                                            ext_access_control: None,
                                        };
                                        target.add_filter(vg_filter);
                                    }
                                    if let Some(url_rewrite_filter) = &filter.url_rewrite {
                                        // Convert Gateway API URLRewrite to Vale Gateway URLRewrite
                                        let mut vg_url_rewrite = vg_core::config::gateway::types::http::filters::URLRewrite {
                                            hostname: url_rewrite_filter.hostname.clone(),
                                            path: None,
                                        };

                                        // Convert path rewrite if present
                                        if let Some(path_config) = &url_rewrite_filter.path {
                                            use vg_core::config::gateway::types::http::filters::{PathRewrite, PathRewriteType};

                                            let vg_path_rewrite = match &path_config.r#type {
                                                gateway_api::apis::standard::httproutes::HTTPRouteRulesFiltersUrlRewritePathType::ReplaceFullPath => {
                                                    PathRewrite {
                                                        rewrite_type: PathRewriteType::ReplaceFullPath,
                                                        replace_full_path: path_config.replace_full_path.clone(),
                                                        replace_prefix_match: None,
                                                    }
                                                }
                                                gateway_api::apis::standard::httproutes::HTTPRouteRulesFiltersUrlRewritePathType::ReplacePrefixMatch => {
                                                    PathRewrite {
                                                        rewrite_type: PathRewriteType::ReplacePrefixMatch,
                                                        replace_full_path: None,
                                                        replace_prefix_match: path_config.replace_prefix_match.clone(),
                                                    }
                                                }
                                            };
                                            vg_url_rewrite.path = Some(vg_path_rewrite);
                                        }

                                        // Add URLRewrite filter to Vale Gateway configuration
                                        let vg_filter = HttpRouteFilter {
                                            filter_type: HttpRouteFilterType::URLRewrite,
                                            request_header_modifier: None,
                                            response_header_modifier: None,
                                            request_mirror: None,
                                            request_redirect: None,
                                            url_rewrite: Some(vg_url_rewrite),
                                            ext_static_response: None,
                                            ext_access_control: None,
                                        };
                                        target.add_filter(vg_filter);
                                    }

                                    if let Some(extension_ref) = &filter.extension_ref {
                                        // Handle extension filters
                                        if extension_ref.group == "vale-gateway.whitefamily.in" {
                                            match ExtensionFilterKind::try_from(extension_ref.kind.as_str()) {
                                                Ok(ExtensionFilterKind::StaticResponseFilter) => {
                                                    let filter_ref = ObjectRef::of_kind::<StaticResponseFilter>()
                                                        .namespace(http_route.metadata.namespace.clone())
                                                        .name(&extension_ref.name)
                                                        .build();

                                                    let static_response = ExtStaticResponseRef::builder()
                                                        .key(filter_ref.to_string())
                                                        .build();

                                                    let vg_filter = HttpRouteFilter {
                                                        filter_type: HttpRouteFilterType::ExtStaticResponse,
                                                        request_header_modifier: None,
                                                        response_header_modifier: None,
                                                        request_mirror: None,
                                                        request_redirect: None,
                                                        url_rewrite: None,
                                                        ext_static_response: Some(static_response),
                                                        ext_access_control: None,
                                                    };

                                                    target.add_filter(vg_filter);
                                                }
                                                Ok(ExtensionFilterKind::AccessControlFilter) => {
                                                    let filter_ref = ObjectRef::of_kind::<AccessControlFilter>()
                                                        .namespace(http_route.metadata.namespace.clone())
                                                        .name(&extension_ref.name)
                                                        .build();

                                                    let access_control = ExtAccessControlRef::builder()
                                                        .key(filter_ref.to_string())
                                                        .build();

                                                    let vg_filter = HttpRouteFilter {
                                                        filter_type: HttpRouteFilterType::ExtAccessControl,
                                                        request_header_modifier: None,
                                                        response_header_modifier: None,
                                                        request_mirror: None,
                                                        request_redirect: None,
                                                        url_rewrite: None,
                                                        ext_static_response: None,
                                                        ext_access_control: Some(access_control),
                                                    };

                                                    target.add_filter(vg_filter);
                                                }
                                                Err(err) => {
                                                    warn!(
                                                        "Unsupported extension filter kind {}: {}",
                                                        extension_ref.kind, err
                                                    );
                                                }
                                            }
                                        } else {
                                            warn!(
                                                "Unsupported extension filter group {} for HTTPRoute {:?} at rule index {}",
                                                extension_ref.group, http_route.metadata.name, index
                                            );
                                        }
                                    }
                                }
                            }

                            // Process matches from HTTPRoute rule
                            if let Some(matches) = &rule.matches {
                                for source in matches {
                                    target.add_match(|target| {
                                        add_method_matches(source, target);
                                        add_path_matches(source, target);
                                        add_header_matches(source, target);
                                        add_query_params_matches(source, target);
                                    });
                                }
                            }

                            // Process backend references
                            if let Some(backend_refs) = &rule.backend_refs {
                                for backend_ref in backend_refs {
                                    let source_ref = ObjectRef::of_kind::<Service>()
                                        .namespace(
                                            backend_ref.namespace.clone().or_else(|| {
                                                http_route.metadata.namespace.clone()
                                            }),
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
                            }
                        });
                    }
                }
            });
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
    for (idx, listener) in instance.gateway().spec.listeners.iter().enumerate() {
        let port = if let Ok(port) = u16::try_from(listener.port) {
            Port::new(port)
        } else {
            warn!(
                "Invalid port {} for listener {} at index {} in gateway {:?}",
                listener.port,
                listener.name,
                idx,
                instance.gateway().metadata.name
            );
            continue;
        };
        gateway_configuration.add_listener(|l| {
            l.with_name(&listener.name)
                .with_port(port)
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

fn set_error_responses_strategy(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    instance: &GatewayInstanceConfiguration,
) {
    let error_responses = instance
        .configuration()
        .error_responses
        .clone()
        .unwrap_or_default();

    let error_responses = match error_responses.kind {
        ErrorResponseKind::Empty => ConfigErrorResponses::builder()
            .kind(ConfigErrorResponseKind::Empty)
            .build(),
        ErrorResponseKind::Html => ConfigErrorResponses::builder()
            .kind(ConfigErrorResponseKind::Html)
            .build(),
        ErrorResponseKind::ProblemDetail => {
            let problem_detail = match error_responses.problem_detail {
                Some(problem_detail) => ProblemDetailErrorResponse::builder()
                    .authority(problem_detail.authority)
                    .build(),
                None => ProblemDetailErrorResponse::default(),
            };

            ConfigErrorResponses::builder()
                .kind(ConfigErrorResponseKind::ProblemDetail)
                .problem_detail(problem_detail)
                .build()
        }
    };

    gateway_configuration.with_error_responses(error_responses);
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
                ClientAddressesSource::Header => match &client_addresses.header {
                    Some(header) => {
                        c.trust_header(header);
                    }
                    None => {
                        warn!("ClientAddressesSource::Header requires a header to be set");
                    }
                },
                ClientAddressesSource::Proxies => {
                    c.trust_proxies(|p| {
                        let Some(proxies) = &client_addresses.proxies else {
                            warn!("ClientAddressesSource::Proxies requires proxies to be set");
                            return;
                        };
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
