use crate::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use getset::Getters;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy};
use k8s_openapi::api::core::v1::{Service, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::DeepMerge;
use kubera_api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use kubera_core::continue_on;
use kubera_core::sync::signal::{channel, Receiver};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;

#[derive(Clone, Debug, Getters, PartialEq)]
pub struct GatewayInstanceConfiguration {
    #[getset(get = "pub")]
    gateway: Arc<Gateway>,

    #[getset(get = "pub")]
    service_overrides: Service,

    #[getset(get = "pub")]
    deployment_overrides: Deployment,
}

pub fn collect_gateway_instances(
    join_set: &mut JoinSet<()>,
    gateways: &Receiver<Objects<Gateway>>,
    gateway_class_parameters: &Receiver<Option<Arc<GatewayClassParameters>>>,
    gateway_parameters: &Receiver<HashMap<ObjectRef, Arc<GatewayParameters>>>,
) -> Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>> {
    let (tx, rx) = channel(HashMap::new());

    let mut gateways = gateways.clone();
    let mut gateway_class_parameters = gateway_class_parameters.clone();
    let mut gateway_parameters = gateway_parameters.clone();

    join_set.spawn(async move {
        loop {
            let current_gateways = gateways.current();
            let current_gateway_class_parameters = gateway_class_parameters.current();
            let current_gateway_parameters = gateway_parameters.current();

            let instances = current_gateways
                .iter()
                .map(|(gateway_ref, _, gateway)| {
                    let gateway_parameters = current_gateway_parameters.get(&gateway_ref).cloned();
                    let deployment_overrides = merge_deployment_overrides(
                        &gateway,
                        current_gateway_class_parameters.as_deref(),
                        gateway_parameters.as_deref(),
                    );
                    let service_overrides = merge_service_overrides(
                        &gateway,
                        current_gateway_class_parameters.as_deref(),
                        gateway_parameters.as_deref(),
                    );
                    (
                        gateway_ref,
                        GatewayInstanceConfiguration {
                            gateway,
                            deployment_overrides,
                            service_overrides,
                        },
                    )
                })
                .collect();

            tx.replace(instances);

            continue_on!(
                gateways.changed(),
                gateway_class_parameters.changed(),
                gateway_parameters.changed()
            );
        }
    });

    rx
}

fn merge_deployment_overrides(
    gateway: &Gateway,
    gateway_class_parameters: Option<&GatewayClassParameters>,
    gateway_parameters: Option<&GatewayParameters>,
) -> Deployment {
    let mut spec = DeploymentSpec::default();

    let class_params = gateway_class_parameters
        .as_ref()
        .and_then(|p| p.spec.common.deployment.as_ref());
    let gateway_params = gateway_parameters
        .and_then(|p| p.spec.common.as_ref())
        .and_then(|c| c.deployment.as_ref());

    spec.replicas = class_params
        .as_ref()
        .and_then(|p| p.replicas)
        .or_else(|| gateway_params.as_ref().and_then(|p| p.replicas));

    if class_params.is_some() || gateway_params.is_some() {
        let mut strategy = DeploymentStrategy::default();

        if let Some(class_strategy) = class_params.and_then(|p| p.strategy.as_ref()) {
            strategy.merge_from(class_strategy.clone());
        }

        if let Some(gateway_strategy) = gateway_params.and_then(|p| p.strategy.as_ref()) {
            strategy.merge_from(gateway_strategy.clone());
        }

        spec.strategy = Some(strategy);
    }

    Deployment {
        spec: Some(spec),
        metadata: merge_metadata(gateway),
        ..Default::default()
    }
}

fn merge_metadata(gateway: &Gateway) -> ObjectMeta {
    let mut metadata = ObjectMeta::default();

    if let Some(infrastructure) = gateway.spec.infrastructure.as_ref() {
        metadata.annotations = infrastructure.annotations.clone();
        metadata.labels = infrastructure.labels.clone();
    }

    metadata
}

fn merge_service_overrides(
    gateway: &Gateway,
    gateway_class_parameters: Option<&GatewayClassParameters>,
    gateway_parameters: Option<&GatewayParameters>,
) -> Service {
    let mut spec = ServiceSpec::default();

    if let Some(param_spec) = gateway_parameters.and_then(|p| p.spec.service.as_ref()) {
        spec.merge_from(param_spec.clone())
    }

    Service {
        spec: Some(spec),
        metadata: merge_metadata(gateway),
        ..Default::default()
    }
}
