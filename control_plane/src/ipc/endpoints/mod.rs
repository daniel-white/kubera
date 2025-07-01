mod get_gateway_configuration;
mod get_gateway_events;

use self::get_gateway_configuration::get_gateway_configuration;
use self::get_gateway_events::get_gateway_events;
use super::IpcServicesState;
use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use problemdetails::Problem;

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
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(state)
}

async fn not_found() -> impl IntoResponse {
    Problem::from(StatusCode::NOT_FOUND)
        .with_title("Not Found")
        .with_detail("The requested resource could not be found")
}

async fn method_not_allowed() -> impl IntoResponse {
    Problem::from(StatusCode::METHOD_NOT_ALLOWED)
        .with_title("Method Not Allowed")
        .with_detail("The requested method is not allowed for this resource")
}
