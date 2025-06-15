mod desired_resources;
mod filters;
mod object_controller;
mod objects;
mod resulting_resources_controller;
mod transformers;

use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::constants::MANAGED_BY_LABEL_QUERY;
use crate::spawn_controller;
use anyhow::Result;
use desired_resources::controller as desired_resources_controller;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::Client;
use kube::runtime::watcher::Config;
use tokio::task::JoinSet;

pub async fn run() -> Result<()> {
    let mut join_set = JoinSet::new();
    let client = Client::try_default().await?;

    let managed_by_selector = Config::default().labels(MANAGED_BY_LABEL_QUERY);

    let gateway_classes = spawn_controller!(GatewayClass, join_set, client);
    let gateway_classes = filters::filter_gateway_classes(&mut join_set, &gateway_classes);
    let gateways = spawn_controller!(Gateway, join_set, client);
    let gateways = filters::filter_gateways(&mut join_set, &gateway_classes, &gateways);
    let http_routes = spawn_controller!(HTTPRoute, join_set, client);
    let http_routes = filters::filter_http_routes(&mut join_set, &gateways, &http_routes);
    let service_backends = transformers::collect_http_route_backends(&mut join_set, &http_routes);
    let endpoint_slices = spawn_controller!(EndpointSlice, join_set, client);
    let service_endpoint_ips =
        transformers::collect_service_backends(&mut join_set, &service_backends, &endpoint_slices);
    let config_maps = spawn_controller!(ConfigMap, join_set, client, managed_by_selector);
    let gateway_config_maps = filters::filter_gateway_config_maps(&mut join_set, &config_maps);
    let gateway_configurations =
        transformers::generate_gateway_configuration(&mut join_set, &gateways);
    transformers::sync_gateway_configuration(
        &mut join_set,
        &client,
        &gateway_config_maps,
        &gateway_configurations,
    );

    // let sources = desired_resources_controller::SourceResourcesReceivers::new_builder()
    //     .gateway_classes(gateway_classes)
    //     .gateway_class_parameters(spawn_controller!(GatewayClassParameters, join_set, client))
    //     .gateways(gateways)
    //     .gateway_parameters(spawn_controller!(GatewayParameters, join_set, client))
    //     .config_maps(spawn_controller!(
    //         ConfigMap,
    //         join_set,
    //         client,
    //         managed_by_selector
    //     ))
    //     .deployments(spawn_controller!(
    //         Deployment,
    //         join_set,
    //         client,
    //         managed_by_selector
    //     ))
    //     .services(spawn_controller!(
    //         Service,
    //         join_set,
    //         client,
    //         managed_by_selector
    //     ))
    //     .namespaces(spawn_controller!(Namespace, join_set, client))
    //     .build()
    //     .expect("Failed to build sources");
    //
    // let desired_resources =
    //     desired_resources_controller::spawn_controller(&mut join_set, sources).await?;
    //
    // resulting_resources_controller::spawn_controller(&mut join_set, &client, desired_resources)
    //     .await?;

    join_set.join_all().await;

    Ok(())
}
