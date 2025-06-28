mod get_gateway_configuration;
mod get_gateway_events;

use self::get_gateway_configuration::get_gateway_configuration;
use self::get_gateway_events::get_gateway_events;
use super::IpcServicesState;
use axum::Router;
use axum::routing::get;

pub fn router(state: IpcServicesState) -> Router {
    Router::new()
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/configuration",
            get(get_gateway_configuration),
        )
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/events",
            get(get_gateway_events),
        )
        .with_state(state)
}
