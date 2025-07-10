use derive_builder::Builder;
use ipnet::IpNet;
use k8s_openapi::api::{apps::v1::DeploymentStrategy, core::v1::ServiceSpec};
use kube::CustomResource;
use schemars::gen::SchemaGenerator;
use schemars::schema::SingleOrVec::Single;
use schemars::schema::{InstanceType, Schema, SchemaObject};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<GatewayConfiguration>,
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

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_addresses: Option<ClientAddresses>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema, IntoStaticStr)]
#[serde(rename_all = "PascalCase")]
#[strum(serialize_all = "PascalCase")]
pub enum ClientAddressesSource {
    #[default]
    None,
    Header,
    Proxies,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddresses {
    pub source: ClientAddressesSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxies: Option<ClientAddressesProxies>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, IntoStaticStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ProxyIpAddressHeaders {
    Forwarded,
    XForwardedFor,
    XForwardedHost,
    XForwardedProto,
    XForwardedBy,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressesProxies {
    #[serde(default = "trusted_private_ranges_default")]
    pub trust_local_ranges: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_ips: Vec<IpAddr>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "cidr_array_schema")]
    pub trusted_ranges: Vec<IpNet>,

    #[serde(
        default = "trusted_headers_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub trusted_headers: Vec<ProxyIpAddressHeaders>,
}

fn trusted_private_ranges_default() -> bool {
    true
}

fn trusted_headers_default() -> Vec<ProxyIpAddressHeaders> {
    vec![ProxyIpAddressHeaders::XForwardedFor]
}

pub fn cidr_array_schema(_: &mut SchemaGenerator) -> Schema {
    // Create schema for a single CIDR
    let item_schema = {
        let schema = SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: Some("cidr".to_string()),
            ..Default::default()
        };
        Schema::Object(schema)
    };

    // Create schema for array of CIDRs
    let mut schema = SchemaObject::default();
    schema.instance_type = Some(InstanceType::Array.into());
    schema.array = Some(Box::new(schemars::schema::ArrayValidation {
        items: Some(Single(Box::new(item_schema))),
        ..Default::default()
    }));

    Schema::Object(schema)
}
