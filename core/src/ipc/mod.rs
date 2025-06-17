use strum::AsRefStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Event {
    Gateway(GatewayEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr)]
#[non_exhaustive]
pub enum GatewayEvent {
    ConfigurationUpdate { namespace: String, name: String },
}
