use derive_builder::Builder;
use k8s_openapi::api::{apps::v1::DeploymentStrategy, core::v1::ServiceSpec};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

#[derive(Default, Builder, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[builder(setter(into))]
pub struct Ref {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GatewayRefs {
    #[default]
    None,
    #[serde(rename = "parentRef")]
    One(Ref),
    #[serde(rename = "parentRefs")]
    Many(Vec<Ref>),
}

#[derive(
    Default, Deserialize, Serialize, Copy, Clone, Debug, JsonSchema, PartialEq, IntoStaticStr,
)]
#[serde(rename_all = "PascalCase")]
#[strum(serialize_all = "camelCase")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Default, Deserialize, Serialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum ImagePullPolicy {
    Always,
    #[default]
    IfNotPresent,
    Never,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct CommonGatewayParameterSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<GatewayDeployment>,
}

#[derive(Default, CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[kube(
    kind = "GatewayClassParameters",
    group = "kubera.whitefamily.in",
    version = "v1alpha1",
    singular = "gateway-class-parameters",
    plural = "gateway-class-parameters"
)]
#[kube(derive = "Default")]
#[kube(derive = "PartialEq")]
pub struct GatewayClassParametersSpec {
    #[serde(flatten)]
    pub common: CommonGatewayParameterSpec,
}

#[derive(Default, CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[kube(
    kind = "GatewayParameters",
    group = "kubera.whitefamily.in",
    version = "v1alpha1",
    namespaced,
    singular = "gateway-parameters",
    plural = "gateway-parameters"
)]
#[kube(derive = "Default")]
#[kube(derive = "PartialEq")]
pub struct GatewayParametersSpec {
    #[serde(flatten)]
    pub common: Option<CommonGatewayParameterSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceSpec>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayDeployment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<DeploymentStrategy>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_pull_policy: Option<ImagePullPolicy>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct GatewayServiceSpec {
    #[serde(flatten)]
    pub spec: ServiceSpec,
}
