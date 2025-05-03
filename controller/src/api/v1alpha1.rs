use k8s_openapi::api::{apps::v1::DeploymentStrategy, core::v1::ServiceSpec};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ref {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GatewayRefs {
    #[serde(rename = "parentRef")]
    One(Ref),
    #[serde(rename = "parentRefs")]
    Many(Vec<Ref>),
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
    pub parent_refs: GatewayRefs,
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
    pub parent_refs: GatewayRefs,
}
