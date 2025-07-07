use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::ObjectRef;
use axum::extract::{Path, Query};
use axum::response::sse::Event;
use axum::{
    extract::State,
    response::{sse::Sse, IntoResponse},
};
use futures::TryStreamExt;
use gateway_api::apis::standard::gateways::Gateway;
use problemdetails::Problem;
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

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
    let gateway_ref = ObjectRef::new_builder()
        .of_kind::<Gateway>()
        .name(path_params.gateway_name)
        .namespace(Some(path_params.gateway_namespace))
        .build()
        .unwrap();

    debug!(
        "Pod {} requesting events for {}",
        query_params.pod_name, gateway_ref
    );

    if !state.gateways.exists(&gateway_ref) {
        return Problem::from(axum::http::StatusCode::NOT_FOUND)
            .with_title("Gateway Not Found")
            .with_detail(format!("Object {gateway_ref} not found"))
            .into_response();
    }

    let stream = state.events.named_gateway_events(gateway_ref).map_ok(|e| {
        Event::default()
            .event(&e)
            .json_data(e.gateway_ref())
            .expect("Failed to serialize event")
    });

    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(state.options().ipc_sse_keep_alive_interval())
                .text("keep-alive"),
        )
        .into_response()
}
