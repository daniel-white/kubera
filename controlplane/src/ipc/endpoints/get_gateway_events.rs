use crate::ipc::IpcServiceState;
use crate::objects::ObjectRef;
use axum::extract::{Path, Query};
use axum::response::sse::Event;
use axum::{
    extract::State,
    response::{sse::Sse, IntoResponse},
};
use futures::TryStreamExt;
use gateway_api::apis::standard::gateways::Gateway;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub struct GetGatewayEventsPathParams {
    gateway_namespace: String,
    gateway_name: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    pod_name: String,
}

pub async fn get_gateway_events(
    State(state): State<IpcServiceState>,
    Path(path_params): Path<GetGatewayEventsPathParams>,
    Query(query_params): Query<QueryParams>,
) -> impl IntoResponse {
    let gateway_ref = ObjectRef::new_builder()
        .of_kind::<Gateway>()
        .name(path_params.gateway_namespace)
        .namespace(Some(path_params.gateway_namespace))
        .build()
        .unwrap();

    let stream = state
        .events
        .named_gateway_events(gateway_ref)
        .map_ok(|e| Event::default().event(e));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
