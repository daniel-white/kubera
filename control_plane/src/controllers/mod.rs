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
use crate::options::Options;
use crate::watch_objects;
use anyhow::Result;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::Getters;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kubera_api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use kubera_core::sync::signal::Receiver;
use std::sync::Arc;
use tokio::task::JoinSet;

#[derive(Getters)]
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

#[derive(Default)]
pub struct SpawnControllersParamsBuilder {
    options: Option<Arc<Options>>,
    kube_client: Option<Receiver<Option<KubeClientCell>>>,
    ipc_services: Option<Arc<IpcServices>>,
    pod_namespace: Option<String>,
    pod_name: Option<String>,
    instance_name: Option<String>,
}

impl SpawnControllersParamsBuilder {
    pub fn options(mut self, options: Arc<Options>) -> Self {
        self.options = Some(options);
        self
    }

    pub fn kube_client(mut self, kube_client: Receiver<Option<KubeClientCell>>) -> Self {
        self.kube_client = Some(kube_client);
        self
    }

    pub fn ipc_services(mut self, ipc_services: Arc<IpcServices>) -> Self {
        self.ipc_services = Some(ipc_services);
        self
    }

    pub fn pod_namespace(mut self, pod_namespace: &String) -> Self {
        self.pod_namespace = Some(pod_namespace.to_string());
        self
    }

    pub fn pod_name(mut self, pod_name: &String) -> Self {
        self.pod_name = Some(pod_name.to_string());
        self
    }

    pub fn instance_name(mut self, instance_name: &String) -> Self {
        self.instance_name = Some(instance_name.to_string());
        self
    }

    pub fn build(self) -> SpawnControllersParams {
        SpawnControllersParams {
            options: self.options.expect("Options are required"),
            kube_client: self.kube_client.expect("Kube client is required"),
            ipc_services: self.ipc_services.expect("IPC services are required"),
            pod_namespace: self.pod_namespace.expect("Pod namespace is required"),
            pod_name: self.pod_name.expect("Pod name is required"),
            instance_name: self.instance_name.expect("Instance name is required"),
        }
    }
}

pub fn spawn_controllers(join_set: &mut JoinSet<()>, params: SpawnControllersParams) {
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

    sync_gateway_configmaps(
        params.options.clone(),
        join_set,
        &kube_client,
        params.ipc_services,
        &instance_role,
        &leader_instance_ip_addr,
        &gateway_instances,
        &http_routes_by_gateway,
        &backends,
    );
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
}
