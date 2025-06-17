use strum::AsRefStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    Gateway(GatewayEvent),
    OtherTBD,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr)]
pub enum GatewayEvent {
    ConfigurationUpdate { namespace: String, name: String },
}
