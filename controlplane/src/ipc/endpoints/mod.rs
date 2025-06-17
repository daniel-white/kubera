mod get_gateway_events;

use self::get_gateway_events::get_gateway_events;
use super::IpcServiceState;
use axum::routing::get;
use axum::Router;

pub fn router(state: IpcServiceState) -> Router {
    Router::new()
        .route(
            "/ipc/gateways/{namespace}/{name}/events",
            get(get_gateway_events),
        )
        .with_state(state)
}
