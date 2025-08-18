use crate::controllers::filters::GatewayClassParametersReferenceState;
use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::apis::standard::gateways::Gateway;
use getset::Getters;
use k8s_openapi::DeepMerge;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy};
use k8s_openapi::api::core::v1::{Service, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use serde_json::{from_value, json};
use std::collections::HashMap;
use std::sync::Arc;
use strum::IntoStaticStr;
use tracing::{info, warn};
use vg_api::v1alpha1::{
    GatewayClassParameters, GatewayConfiguration, GatewayParameters,
    ImagePullPolicy as ApiImagePullPolicy,
};
use vg_core::continue_on;
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

#[derive(Default, Copy, Clone, Debug, PartialEq, IntoStaticStr)]
#[strum(serialize_all = "PascalCase")]
pub enum ImagePullPolicy {
    Always,
    #[default]
    IfNotPresent,
    Never,
}

impl From<ApiImagePullPolicy> for ImagePullPolicy {
    fn from(policy: ApiImagePullPolicy) -> Self {
        match policy {
            ApiImagePullPolicy::Always => ImagePullPolicy::Always,
            ApiImagePullPolicy::IfNotPresent => ImagePullPolicy::IfNotPresent,
            ApiImagePullPolicy::Never => ImagePullPolicy::Never,
        }
    }
}

#[derive(Clone, Debug, Getters, PartialEq)]
pub struct GatewayInstanceConfiguration {
    #[getset(get = "pub")]
    gateway: Arc<Gateway>,

    #[getset(get = "pub")]
    service_overrides: Service,

    #[getset(get = "pub")]
    deployment_overrides: Deployment,

    #[getset(get = "pub")]
    image_pull_policy: ImagePullPolicy,

    #[getset(get = "pub")]
    image_repository: String,

    #[getset(get = "pub")]
    image_tag: String,

    #[getset(get = "pub")]
    configuration: GatewayConfiguration,
}

pub fn collect_gateway_instances(
    task_builder: &TaskBuilder,
    gateways_rx: &Receiver<Objects<Gateway>>,
    gateway_class_parameters_rx: &Receiver<GatewayClassParametersReferenceState>,
    gateway_parameters_rx: &Receiver<Objects<GatewayParameters>>,
) -> Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>> {
    let (tx, rx) = signal("collected_gateway_instances");

    let gateways_rx = gateways_rx.clone();
    let gateway_class_parameters_rx = gateway_class_parameters_rx.clone();
    let gateway_parameters_rx = gateway_parameters_rx.clone();

    task_builder
        .new_task(stringify!(collect_gateway_instances))
        .spawn(async move {
            loop {
                await_ready!(
                    gateways_rx,
                    gateway_class_parameters_rx,
                    gateway_parameters_rx,
                )
                .and_then(
                    async |gateways, gateway_class_parameters, gateway_parameters| {
                        info!("Collecting gateway instances");
                        let gateway_class_parameters: Option<Arc<GatewayClassParameters>> =
                            gateway_class_parameters.into();
                        let gateway_class_parameters = gateway_class_parameters.as_deref();

                        let instances = gateways
                            .iter()
                            .map(|(gateway_ref, _, gateway)| {
                                info!("Processing gateway instance: {}", gateway_ref);
                                let gateway_parameters =
                                    gateway_parameters.get_by_ref(&gateway_ref);
                                let gateway_parameters = gateway_parameters.as_deref();
                                let (
                                    deployment_overrides,
                                    image_pull_policy,
                                    image_repository,
                                    image_tag,
                                ) = merge_deployment_overrides(
                                    &gateway,
                                    gateway_class_parameters,
                                    gateway_parameters,
                                );
                                let service_overrides = merge_service_overrides(
                                    &gateway,
                                    gateway_class_parameters,
                                    gateway_parameters,
                                );
                                let configuration = merge_gateway_configuration(
                                    &gateway,
                                    gateway_class_parameters,
                                    gateway_parameters,
                                );
                                (
                                    gateway_ref,
                                    GatewayInstanceConfiguration {
                                        gateway,
                                        service_overrides,
                                        deployment_overrides,
                                        image_pull_policy,
                                        image_repository,
                                        image_tag,
                                        configuration,
                                    },
                                )
                            })
                            .collect();

                        tx.set(instances).await;
                    },
                )
                .run()
                .await;

                continue_on!(
                    gateways_rx.changed(),
                    gateway_class_parameters_rx.changed(),
                    gateway_parameters_rx.changed()
                );
            }
        });

    rx
}

fn merge_deployment_overrides(
    gateway: &Gateway,
    gateway_class_parameters: Option<&GatewayClassParameters>,
    gateway_parameters: Option<&GatewayParameters>,
) -> (Deployment, ImagePullPolicy, String, String) {
    let mut spec = DeploymentSpec::default();

    let class_deployment_params = gateway_class_parameters
        .as_ref()
        .and_then(|p| p.spec.common.deployment.as_ref());
    let deployment_params = gateway_parameters
        .and_then(|p| p.spec.common.as_ref())
        .and_then(|c| c.deployment.as_ref());

    // Set replicas: gateway parameters > class parameters > default
    spec.replicas = deployment_params
        .and_then(|p| p.replicas)
        .or_else(|| class_deployment_params.and_then(|p| p.replicas));

    let class_gateway_params = gateway_class_parameters
        .as_ref()
        .and_then(|p| p.spec.common.gateway.as_ref());
    let gateway_params = gateway_parameters
        .and_then(|p| p.spec.common.as_ref())
        .and_then(|c| c.gateway.as_ref());

    let image_pull_policy = class_deployment_params
        .and_then(|p| p.image_pull_policy)
        .or_else(|| deployment_params.and_then(|p| p.image_pull_policy))
        .unwrap_or_default()
        .into();

    // Extract image configuration with precedence: gateway-level params > class-level params > defaults
    let image_repository = deployment_params
        .and_then(|p| p.image.as_ref())
        .and_then(|img| img.repository.as_ref())
        .or_else(|| {
            class_deployment_params
                .and_then(|p| p.image.as_ref())
                .and_then(|img| img.repository.as_ref())
        })
        .cloned()
        .unwrap_or_else(|| "vale-gateway".to_string());

    let image_tag = deployment_params
        .and_then(|p| p.image.as_ref())
        .and_then(|img| img.tag.as_ref())
        .or_else(|| {
            class_deployment_params
                .and_then(|p| p.image.as_ref())
                .and_then(|img| img.tag.as_ref())
        })
        .cloned()
        .unwrap_or_else(|| "latest".to_string());

    if class_deployment_params.is_some() || deployment_params.is_some() {
        let mut strategy = DeploymentStrategy::default();

        if let Some(class_strategy) = class_deployment_params.and_then(|p| p.strategy.as_ref()) {
            strategy.merge_from(class_strategy.clone());
        }

        if let Some(gateway_strategy) = deployment_params.and_then(|p| p.strategy.as_ref()) {
            strategy.merge_from(gateway_strategy.clone());
        }

        spec.strategy = Some(strategy);
    }

    // Optionally set the log level for the gateway container
    let log_level = gateway_params
        .and_then(|p| p.log_level)
        .or_else(|| class_gateway_params.and_then(|p| p.log_level));

    #[allow(clippy::expect_used)]
    if let Some(log_level) = log_level {
        let log_level: &'static str = log_level.into();

        let template_spec = json!({
            "containers": [{
                "name": "gateway",
                "env": [{
                    "name": "RUST_LOG",
                    "value": log_level,
                }]
            }],
        });

        spec.template.spec =
            from_value(template_spec).expect("Failed to parse containers template spec");
    }

    (
        Deployment {
            spec: Some(spec),
            metadata: merge_metadata(gateway),
            ..Default::default()
        },
        image_pull_policy,
        image_repository,
        image_tag,
    )
}

fn merge_metadata(gateway: &Gateway) -> ObjectMeta {
    let mut metadata = ObjectMeta::default();

    if let Some(infrastructure) = gateway.spec.infrastructure.as_ref() {
        metadata.annotations.clone_from(&infrastructure.annotations);
        metadata.labels.clone_from(&infrastructure.labels);
    }

    metadata
}

fn merge_service_overrides(
    gateway: &Gateway,
    _gateway_class_parameters: Option<&GatewayClassParameters>,
    gateway_parameters: Option<&GatewayParameters>,
) -> Service {
    let mut spec = ServiceSpec::default();

    if let Some(param_spec) = gateway_parameters.and_then(|p| p.spec.service.as_ref()) {
        spec.merge_from(param_spec.clone());
    }

    Service {
        spec: Some(spec),
        metadata: merge_metadata(gateway),
        ..Default::default()
    }
}

fn merge_gateway_configuration(
    _gateway: &Gateway,
    gateway_class_parameters: Option<&GatewayClassParameters>,
    gateway_parameters: Option<&GatewayParameters>,
) -> GatewayConfiguration {
    let gateway_class = gateway_class_parameters.and_then(|p| p.spec.common.gateway.as_ref());
    let gateway = gateway_parameters
        .and_then(|p| p.spec.common.as_ref())
        .and_then(|c| c.gateway.as_ref());
    warn!(
        "Using gateway configuration: {:?} {:?}",
        gateway_class, gateway
    );
    match (gateway_class, gateway) {
        (_, Some(gateway)) => gateway.clone(),
        (Some(gateway_class), _) => gateway_class.clone(),
        _ => GatewayConfiguration::default(),
    }
}
