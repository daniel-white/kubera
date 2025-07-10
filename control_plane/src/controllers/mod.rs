mod filters;
mod instances;
mod macros;
mod sync;
mod transformers;

use self::filters::{
    filter_gateway_class_parameters, filter_gateway_classes, filter_gateway_parameters,
    filter_gateways, filter_http_routes,
};
use self::sync::{
    sync_gateway_configmaps, sync_gateway_deployments, sync_gateway_services,
    SyncGatewayConfigmapsParams, SyncGatewayConfigmapsParamsBuilderError,
};
use self::transformers::{
    collect_gateway_instances, collect_http_route_backends, collect_http_routes_by_gateway,
    collect_service_backends,
};
use crate::controllers::instances::{determine_instance_role, watch_leader_instance_ip_addr};
use crate::ipc::IpcServices;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use anyhow::Result;
use derive_builder::Builder;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::Getters;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kubera_api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use kubera_core::sync::signal::Receiver;
use std::sync::Arc;
use thiserror::Error;
use tokio::task::JoinSet;

#[derive(Getters, Builder)]
#[builder(setter(into))]
pub struct SpawnControllersParams {
    options: Arc<Options>,
    kube_client: Receiver<Option<KubeClientCell>>,
    ipc_services: Arc<IpcServices>,
    pod_namespace: String,
    pod_name: String,
    instance_name: String,
}

impl SpawnControllersParams {
    pub fn new_builder() -> SpawnControllersParamsBuilder {
        SpawnControllersParamsBuilder::default()
    }
}

#[derive(Debug, Error)]
pub enum SpawnControllersError {
    #[error("Failed to build SyncGatewayConfigmapsParams: {0}")]
    SyncGatewayConfigmapsParams(#[from] SyncGatewayConfigmapsParamsBuilderError),
}

pub fn spawn_controllers(
    join_set: &mut JoinSet<()>,
    params: SpawnControllersParams,
) -> Result<(), SpawnControllersError> {
    let options = params.options.clone();
    let kube_client = params.kube_client;

    let instance_role = determine_instance_role(
        options.clone(),
        join_set,
        &params.pod_namespace,
        &params.instance_name,
        &params.pod_name,
    );
    let leader_instance_ip_addr =
        watch_leader_instance_ip_addr(options.clone(), join_set, &kube_client, &instance_role);

    let gateway_classes = watch_objects!(options, join_set, GatewayClass, kube_client);
    let gateways = watch_objects!(options, join_set, Gateway, kube_client);
    let http_routes = watch_objects!(options, join_set, HTTPRoute, kube_client);
    let endpoint_slices = watch_objects!(options, join_set, EndpointSlice, kube_client);
    let gateway_class_parameters =
        watch_objects!(options, join_set, GatewayClassParameters, kube_client);
    let gateway_parameters = watch_objects!(options, join_set, GatewayParameters, kube_client);

    let gateway_class = filter_gateway_classes(join_set, &gateway_classes);
    let gateway_class_parameters =
        filter_gateway_class_parameters(join_set, &gateway_class, &gateway_class_parameters);
    let gateways = filter_gateways(join_set, &gateway_class, &gateways);
    let gateway_parameters = filter_gateway_parameters(join_set, &gateways, &gateway_parameters);
    let gateway_instances = collect_gateway_instances(
        join_set,
        &gateways,
        &gateway_class_parameters,
        &gateway_parameters,
    );
    let http_routes = filter_http_routes(join_set, &gateways, &http_routes);
    let http_routes_by_gateway = collect_http_routes_by_gateway(join_set, &gateways, &http_routes);
    let service_backends = collect_http_route_backends(join_set, &http_routes);
    let backends = collect_service_backends(join_set, &service_backends, &endpoint_slices);

    {
        let params = SyncGatewayConfigmapsParams::new_builder()
            .options(options.clone())
            .kube_client(kube_client.clone())
            .ipc_services(params.ipc_services.clone())
            .instance_role(instance_role.clone())
            .primary_instance_ip_addr(leader_instance_ip_addr.clone())
            .gateway_instances(gateway_instances.clone())
            .http_routes(http_routes_by_gateway.clone())
            .backends(backends.clone())
            .build()?;

        sync_gateway_configmaps(join_set, params);
    }

    sync_gateway_services(
        params.options.clone(),
        join_set,
        &kube_client,
        &instance_role,
        &gateway_instances,
    );
    sync_gateway_deployments(
        params.options.clone(),
        join_set,
        &kube_client,
        &instance_role,
        &gateway_instances,
    );

    Ok(())
}
