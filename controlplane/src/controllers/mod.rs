mod filters;
mod macros;
mod sync;
mod transformers;

use self::filters::*;
use self::sync::*;
use self::transformers::*;
use crate::controllers::sync::sync_gateway_configmaps;
use crate::ipc::IpcServices;
use crate::objects::{ObjectRef, SyncObjectAction};
use crate::{sync_objects, watch_objects};
use anyhow::Result;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::runtime::watcher::Config;
use kube::Client;
use kubera_api::constants::MANAGED_BY_LABEL_QUERY;
use std::sync::Arc;
use tokio::signal::ctrl_c;

pub async fn run(ipc_services: IpcServices) -> Result<()> {
    let ipc_services = Arc::new(ipc_services);
    let client = Client::try_default().await?;

    let managed_by_selector = Config::default().labels(MANAGED_BY_LABEL_QUERY);

    let gateway_classes = watch_objects!(GatewayClass, client);
    let gateways = watch_objects!(Gateway, client);
    let http_routes = watch_objects!(HTTPRoute, client);
    let endpoint_slices = watch_objects!(EndpointSlice, client);

    let gateway_classes = filter_gateway_classes(&gateway_classes);
    let gateways = filter_gateways(&gateway_classes, &gateways);
    let http_routes = filter_http_routes(&gateways, &http_routes);
    let http_routes_by_gateway = collect_http_routes_by_gateway(&gateways, &http_routes);
    let service_backends = collect_http_route_backends(&http_routes);
    let backends = collect_service_backends(&service_backends, &endpoint_slices);

    sync_gateway_configmaps(&client, &gateways, &http_routes_by_gateway, &backends);
    sync_gateway_services(&client, &gateways);
    sync_gateway_deployments(&client, &gateways);

    let _ = ctrl_c().await;

    Ok(())
}
