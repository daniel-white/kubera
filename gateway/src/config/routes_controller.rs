use crate::http::router::{Route, RouteMatcher};
use derive_builder::Builder;
use getset::Getters;
use http::{HeaderValue, Method};
use kubera_core::config::gateway::types::{
    GatewayConfiguration, HostnameMatchType, HttpHeaderMatchType, HttpPathMatchType,
    HttpQueryParamNameMatchType,
};
use kubera_core::select_continue;
use kubera_core::sync::signal::{Receiver, channel};
use thiserror::Error;
use tracing::{Level, debug, span, trace};

#[derive(Default, Getters, Debug, Clone, PartialEq)]
pub struct Routes(#[getset(get = "pub")] Vec<Route>);

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("Failed to spawn controller")]
    SpawnError,
}

pub async fn spawn_controller(
    gateway_configuration: Receiver<Option<GatewayConfiguration>>,
) -> Result<Receiver<Routes>, ControllerError> {
    let mut gateway_configuration = gateway_configuration.clone();
    let (tx, rx) = channel(Routes::default());

    tokio::spawn(async move {
        loop {
            if let Some(config) = gateway_configuration.current() {}

            select_continue!(gateway_configuration.changed())
        }
    });

    Ok(rx)
}
