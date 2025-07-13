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
    SyncGatewayConfigmapsParams, SyncGatewayConfigmapsParamsBuilderError, sync_gateway_configmaps,
    sync_gateway_deployments, sync_gateway_services,
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
    kube_client_rx: Receiver<KubeClientCell>,
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
    let kube_client_rx = params.kube_client_rx;

    let instance_role_rx = determine_instance_role(
        options.clone(),
        join_set,
        &params.pod_namespace,
        &params.instance_name,
        &params.pod_name,
    );
    let leader_instance_ip_addr_rx = watch_leader_instance_ip_addr(
        options.clone(),
        join_set,
        &kube_client_rx,
        &instance_role_rx,
    );

    let gateway_classes_rx = watch_objects!(options, join_set, GatewayClass, kube_client_rx);
    let gateways_rx = watch_objects!(options, join_set, Gateway, kube_client_rx);
    let http_routes_rx = watch_objects!(options, join_set, HTTPRoute, kube_client_rx);
    let endpoint_slices_rx = watch_objects!(options, join_set, EndpointSlice, kube_client_rx);
    let gateway_class_parameters_rx =
        watch_objects!(options, join_set, GatewayClassParameters, kube_client_rx);
    let gateway_parameters_rx =
        watch_objects!(options, join_set, GatewayParameters, kube_client_rx);

    let gateway_class_rx = filter_gateway_classes(join_set, &gateway_classes_rx);
    let gateway_class_parameters_rx =
        filter_gateway_class_parameters(join_set, &gateway_class_rx, &gateway_class_parameters_rx);
    let gateways_rx = filter_gateways(join_set, &gateway_class_rx, &gateways_rx);
    let gateway_parameters_rx =
        filter_gateway_parameters(join_set, &gateways_rx, &gateway_parameters_rx);
    let gateway_instances_rx = collect_gateway_instances(
        join_set,
        &gateways_rx,
        &gateway_class_parameters_rx,
        &gateway_parameters_rx,
    );
    let http_routes_rx = filter_http_routes(join_set, &gateways_rx, &http_routes_rx);
    let http_routes_by_gateway_rx = collect_http_routes_by_gateway(join_set, &http_routes_rx);
    let service_backends_rx = collect_http_route_backends(join_set, &http_routes_rx);
    let backends_rx = collect_service_backends(join_set, &service_backends_rx, &endpoint_slices_rx);

    {
        let params = SyncGatewayConfigmapsParams::new_builder()
            .options(options.clone())
            .kube_client_rx(kube_client_rx.clone())
            .ipc_services(params.ipc_services.clone())
            .instance_role_rx(instance_role_rx.clone())
            .primary_instance_ip_addr_rx(leader_instance_ip_addr_rx.clone())
            .gateway_instances_rx(gateway_instances_rx.clone())
            .http_routes_rx(http_routes_by_gateway_rx.clone())
            .backends_rx(backends_rx)
            .build()?;

        sync_gateway_configmaps(join_set, params);
    }

    sync_gateway_services(
        params.options.clone(),
        join_set,
        &kube_client_rx,
        &instance_role_rx,
        &gateway_instances_rx,
    );
    sync_gateway_deployments(
        params.options.clone(),
        join_set,
        &kube_client_rx,
        &instance_role_rx,
        &gateway_instances_rx,
    );

    Ok(())
}
