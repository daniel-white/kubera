use strum::AsRefStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr)]
pub enum GatewayEventType {
    ConfigurationUpdated,
}
