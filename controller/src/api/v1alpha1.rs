use k8s_openapi::api::{apps::v1::DeploymentStrategy, core::v1::ServiceSpec};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct GatewayRefs {
    #[serde(
        rename = "gatewayRefs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub refs: Option<Vec<String>>,

    #[serde(
        rename = "gatewayRef",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub ref_: Option<String>,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    kind = "GatewayDeployment",
    group = "kubera.whitefamily.in",
    version = "v1alpha1",
    namespaced
)]
pub struct GatewayDeploymentSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<DeploymentStrategy>,

    #[serde(flatten)]
    pub gateway_refs: GatewayRefs,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    kind = "GatewayService",
    group = "kubera.whitefamily.in",
    version = "v1alpha1",
    namespaced
)]
pub struct GatewayServiceSpec {
    #[serde(flatten)]
    pub spec: ServiceSpec,

    #[serde(flatten)]
    pub gateway_refs: GatewayRefs,
}
