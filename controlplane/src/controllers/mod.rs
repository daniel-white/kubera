mod filters;
mod macros;
mod sync;
mod transformers;

use self::filters::*;
use self::sync::*;
use self::transformers::*;
use crate::ipc::IpcServices;
use crate::watch_objects;
use anyhow::Result;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::runtime::watcher::Config;
use kube::Client;
use kubera_api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use std::sync::Arc;
use tokio::task::JoinSet;

pub async fn run(ipc_services: IpcServices) -> Result<()> {
    let ipc_services = Arc::new(ipc_services);
    let client = Client::try_default().await?;

    let mut join_set: JoinSet<_> = JoinSet::new();

    let gateway_classes = watch_objects!(join_set, GatewayClass, client);
    let gateways = watch_objects!(join_set, Gateway, client);
    let http_routes = watch_objects!(join_set, HTTPRoute, client);
    let endpoint_slices = watch_objects!(join_set, EndpointSlice, client);
    let gateway_class_parameters = watch_objects!(join_set, GatewayClassParameters, client);
    let gateway_parameters = watch_objects!(join_set, GatewayParameters, client);

    let gateway_class = filter_gateway_classes(&mut join_set, &gateway_classes);
    let gateway_class_parameters =
        filter_gateway_class_parameters(&mut join_set, &gateway_class, &gateway_class_parameters);
    let gateways = filter_gateways(&mut join_set, &gateway_class, &gateways);
    let gateway_parameters =
        filter_gateway_parameters(&mut join_set, &gateways, &gateway_parameters);
    let gateway_instances = collect_gateway_instances(
        &mut join_set,
        &gateways,
        &gateway_class_parameters,
        &gateway_parameters,
    );
    let http_routes = filter_http_routes(&mut join_set, &gateways, &http_routes);
    let http_routes_by_gateway =
        collect_http_routes_by_gateway(&mut join_set, &gateways, &http_routes);
    let service_backends = collect_http_route_backends(&mut join_set, &http_routes);
    let backends = collect_service_backends(&mut join_set, &service_backends, &endpoint_slices);

    sync_gateway_configmaps(
        &mut join_set,
        &client,
        ipc_services,
        &gateways,
        &http_routes_by_gateway,
        &backends,
    );
    sync_gateway_services(&mut join_set, &client, &gateways);
    sync_gateway_deployments(&mut join_set, &client, &gateways);

    join_set.join_all().await;

    Ok(())
}
