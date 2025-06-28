use derive_builder::Builder;
use getset::Getters;
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
}
