use crate::http::listeners::{HttpListener, HttpListenerBuilder};
use crate::ipc::IpcConfiguration;
use ::serde::{Deserialize, Serialize};
use getset::{CloneGetters, CopyGetters, Getters};
use schemars::JsonSchema;
use serde_valid::Validate;
use strum::EnumString;

#[derive(
    Validate,
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    Hash,
    EnumString,
)]
#[serde(rename_all = "lowercase")]
pub enum GatewayVersion {
    #[default]
    #[serde(rename = "v1alpha1")]
    V1Alpha1,
}

#[derive(
    Validate,
    Getters,
    CloneGetters,
    CopyGetters,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct Gateway {
    #[getset(get_copy = "pub")]
    version: GatewayVersion,

    #[getset(get = "pub")]
    ipc: IpcConfiguration,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    http_listener: Option<HttpListener>,
}

impl Gateway {
    pub fn builder() -> GatewayBuilder {
        GatewayBuilder {
            ipc: None,
            http_listener_builder: None,
        }
    }
}

#[derive(Debug)]
pub struct GatewayBuilder {
    ipc: Option<IpcConfiguration>,
    http_listener_builder: Option<HttpListenerBuilder>,
}

impl GatewayBuilder {
    pub fn build(self) -> Gateway {
        Gateway {
            version: GatewayVersion::V1Alpha1,
            ipc: self.ipc.expect("IPC configuration is required"),
            http_listener: self.http_listener_builder.map(|b| b.build()),
        }
    }

    pub fn with_ipc(&mut self, ipc: IpcConfiguration) -> &mut Self {
        self.ipc = Some(ipc);
        self
    }

    pub fn with_http_listener<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpListenerBuilder),
    {
        let mut listener_builder = HttpListener::builder();
        factory(&mut listener_builder);
        self.http_listener_builder = Some(listener_builder);
        self
    }
}
