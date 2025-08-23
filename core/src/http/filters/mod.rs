use self::access_control::AccessControlFilter;
use self::client_addrs::ClientAddrsFilter;
use self::header_modifier::HeaderModifierFilter;
use crate::http::filters::access_control::AccessControlFilterRef;
use crate::http::filters::client_addrs::ClientAddrsFilterRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

pub mod access_control;
pub mod client_addrs;
pub mod header_modifier;

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub enum RouteRuleFilter {
    UpstreamRequestHeaderModifier(HeaderModifierFilter),
    ResponseHeaderModifier(HeaderModifierFilter),
    Gateway(GatewayFilterRef),
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub enum GatewayFilterRef {
    AccessControl(AccessControlFilterRef),
    ClientAddrs(ClientAddrsFilterRef),
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "pascalCase")]
pub enum GatewayFilter {
    AccessControl(AccessControlFilter),
    ClientAddrs(ClientAddrsFilter),
}
