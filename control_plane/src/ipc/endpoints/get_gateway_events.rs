use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::ObjectRef;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::sse::Event;
use axum::{
    extract::State,
    response::{IntoResponse, sse::Sse},
};
use futures::TryStreamExt;
use gateway_api::apis::standard::gateways::Gateway;
use problemdetails::Problem;
use serde::Deserialize;
use tracing::{debug, warn};

#[derive(Deserialize)]
pub struct PathParams {
    gateway_namespace: String,
    gateway_name: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    pod_name: String,
}

pub async fn get_gateway_events(
    State(state): State<IpcEndpointState>,
    Path(path_params): Path<PathParams>,
    Query(query_params): Query<QueryParams>,
) -> impl IntoResponse {
    if path_params.gateway_namespace.is_empty() {
        return Problem::from(StatusCode::BAD_REQUEST)
            .with_title("Invalid Namespace")
            .with_detail("Gateway namespace cannot be empty")
            .into_response();
    }
    if path_params.gateway_name.is_empty() {
        return Problem::from(StatusCode::BAD_REQUEST)
            .with_title("Invalid Name")
            .with_detail("Gateway name cannot be empty")
            .into_response();
    }
    
    let gateway_ref = ObjectRef::of_kind::<Gateway>()
        .name(path_params.gateway_name)
        .namespace(Some(path_params.gateway_namespace))
        .build();

    debug!(
        "Pod {} requesting events for {:?}",
        query_params.pod_name, gateway_ref
    );

    if !state.gateways.exists(&gateway_ref) {
        return Problem::from(StatusCode::NOT_FOUND)
            .with_title("Gateway Not Found")
            .with_detail(format!("Object {gateway_ref} not found"))
            .into_response();
    }

    let stream = state
        .events
        .named_gateway_events(gateway_ref)
        .map_ok(|event| {
            Event::default()
                .event(&event)
                .json_data(event.gateway_ref())
                .unwrap_or_else(|err| {
                    warn!("Failed to serialize event for SSE: {err}");
                    Event::default().comment("keep-alive")
                })
        });

    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(state.options().ipc_sse_keep_alive_interval())
                .text("keep-alive"),
        )
        .into_response()
}
