use derive_builder::Builder;
use getset::Getters;
use schemars::_private::serde_json;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, IntoStaticStr};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Event {
    Gateway(GatewayEvent),
}

#[derive(Debug, Clone, Builder, PartialEq, Serialize, Deserialize, Getters, Eq, Hash)]
#[builder(setter(into))]
pub struct Ref {
    #[getset(get = "pub")]
    namespace: String,

    #[getset(get = "pub")]
    name: String,
}

impl Ref {
    pub fn new_builder() -> RefBuilder {
        RefBuilder::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, IntoStaticStr, AsRefStr)]
#[non_exhaustive]
#[strum(serialize_all = "snake_case")]
pub enum GatewayEvent {
    ConfigurationUpdate(Ref),
    Deleted(Ref),
}

impl GatewayEvent {
    pub fn gateway_ref(&self) -> &Ref {
        match self {
            GatewayEvent::ConfigurationUpdate(gateway_ref) => gateway_ref,
            GatewayEvent::Deleted(gateway_ref) => gateway_ref,
        }
    }

    pub fn try_parse<E: AsRef<str>, D: AsRef<str>>(event: E, data: D) -> Result<Self, String> {
        match event.as_ref() {
            "configuration_update" => {
                let ref_ = serde_json::from_str(data.as_ref()).map_err(|e| {
                    format!("Failed to parse GatewayEvent::ConfigurationUpdate: {}", e)
                })?;
                Ok(GatewayEvent::ConfigurationUpdate(ref_))
            }
            "deleted" => {
                let ref_ = serde_json::from_str(data.as_ref())
                    .map_err(|e| format!("Failed to parse GatewayEvent::Deleted: {}", e))?;
                Ok(GatewayEvent::Deleted(ref_))
            }
            _ => Err(format!("Unknown event type: {}", event.as_ref())),
        }
    }
}
