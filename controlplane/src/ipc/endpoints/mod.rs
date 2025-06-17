mod get_gateway_configuration;
mod get_gateway_events;

use self::get_gateway_configuration::get_gateway_configuration;
use self::get_gateway_events::get_gateway_events;
use super::IpcServiceState;
use axum::Router;
use axum::routing::get;

pub fn router(state: IpcServiceState) -> Router {
    Router::new()
        .route(
            "/ipc/gateways/{namespace}/{name}/configuration",
            get(get_gateway_configuration),
        )
        .route(
            "/ipc/gateways/{namespace}/{name}/events",
            get(get_gateway_events),
        )
        .with_state(state)
}
