use strum::{AsRefStr, IntoStaticStr};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Event {
    Gateway(GatewayEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, IntoStaticStr, AsRefStr)]
#[non_exhaustive]
pub enum GatewayEvent {
    ConfigurationUpdate { namespace: String, name: String },
}
