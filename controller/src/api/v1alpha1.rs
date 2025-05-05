use derive_builder::Builder;
use k8s_openapi::api::{apps::v1::DeploymentStrategy, core::v1::ServiceSpec};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Builder, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[builder(setter(into))]
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

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct CommonGatewayParameters {}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    kind = "GatewayClassParameters",
    group = "kubera.whitefamily.in",
    version = "v1alpha1"
)]
pub struct GatewayClassConfigurationSpec {
    #[serde(flatten)]
    pub common: CommonGatewayParameters,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    kind = "GatewayParameters",
    group = "kubera.whitefamily.in",
    version = "v1alpha1",
    namespaced
)]
pub struct GatewayParametersSpec {
    #[serde(flatten)]
    pub common: Option<CommonGatewayParameters>,

    #[serde(flatten)]
    pub parent_refs: GatewayRefs,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<GatewayProxyConfiguration>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct GatewayProxyConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<GatewayDeployment>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceSpec>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct GatewayDeployment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<DeploymentStrategy>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct GatewayServiceSpec {
    #[serde(flatten)]
    pub spec: ServiceSpec,
}
