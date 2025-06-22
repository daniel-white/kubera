mod filters;
mod macros;
mod transformers;

use self::filters::*;
use self::transformers::*;
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
    let managed_config_maps = watch_objects!(ConfigMap, client, managed_by_selector);
    let managed_deployments = watch_objects!(Deployment, client, managed_by_selector);
    let managed_services = watch_objects!(Service, client, managed_by_selector);

    let gateway_classes = filter_gateway_classes(&gateway_classes);
    let gateways = filter_gateways(&gateway_classes, &gateways);
    let http_routes = filter_http_routes(&gateways, &http_routes);
    let http_routes_by_gateway = collect_http_routes_by_gateway(&gateways, &http_routes);
    let service_backends = collect_http_route_backends(&http_routes);

    let backends = collect_service_backends(&service_backends, &endpoint_slices);

    let gateway_config_maps = filter_gateway_config_maps(&managed_config_maps);
    let gateway_configurations = generate_gateway_configuration(
        &gateways,
        &http_routes_by_gateway,
        &backends,
        ipc_services.clone(),
    );
    sync_gateway_configuration(&client, &gateway_config_maps, &gateway_configurations);
    generate_gateway_services(&gateways, ipc_services.clone());

    let (tx, rx) = tokio::sync::broadcast::channel(100);

    sync_objects!(Service, client, tx, String, "service: {{.}}");

    let r = ObjectRef::new_builder()
        .of_kind::<Service>()
        .namespace(Some("default".to_string()))
        .name("echo-server")
        .build()
        .expect("Failed to build ObjectRef");

    let msg = SyncObjectAction::Delete(r);

    tx.send(msg).expect("Failed to send message");

    ctrl_c().await;

    Ok(())
}
