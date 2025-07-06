mod filters;
mod instances;
mod macros;
mod sync;
mod transformers;

use self::filters::*;
use self::sync::*;
use self::transformers::*;
use crate::controllers::instances::{determine_instance_role, watch_leader_instance_ip_addr};
use crate::ipc::IpcServices;
use crate::kubernetes::KubeClientCell;
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
use tokio::task::JoinSet;

#[derive(Builder, Getters, Clone)]
#[builder(setter(into))]
pub struct SpawnControllersParams {
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

pub fn spawn_controllers(join_set: &mut JoinSet<()>, params: SpawnControllersParams) {
    let kube_client = params.kube_client;

    let instance_role = determine_instance_role(
        join_set,
        &params.pod_namespace,
        &params.instance_name,
        &params.pod_name,
    );
    let leader_instance_ip_addr =
        watch_leader_instance_ip_addr(join_set, &kube_client, &instance_role);

    let gateway_classes = watch_objects!(join_set, GatewayClass, kube_client);
    let gateways = watch_objects!(join_set, Gateway, kube_client);
    let http_routes = watch_objects!(join_set, HTTPRoute, kube_client);
    let endpoint_slices = watch_objects!(join_set, EndpointSlice, kube_client);
    let gateway_class_parameters = watch_objects!(join_set, GatewayClassParameters, kube_client);
    let gateway_parameters = watch_objects!(join_set, GatewayParameters, kube_client);

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

    sync_gateway_configmaps(
        join_set,
        &kube_client,
        params.ipc_services,
        &instance_role,
        &leader_instance_ip_addr,
        &gateway_instances,
        &http_routes_by_gateway,
        &backends,
    );
    sync_gateway_services(join_set, &kube_client, &instance_role, &gateway_instances);
    sync_gateway_deployments(join_set, &kube_client, &instance_role, &gateway_instances);
}
