use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::ObjectRef;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::sse::Event;
use axum::{
    extract::State,
    response::{sse::Sse, IntoResponse},
};
use futures::TryStreamExt;
use gateway_api::apis::standard::gateways::Gateway;
use problemdetails::Problem;
use serde::Deserialize;
use tracing::{debug, instrument, warn};
use vg_core::instrumentation::trace_id;

#[derive(Deserialize, Debug)]
pub struct PathParams {
    gateway_namespace: String,
    gateway_name: String,
}

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pod_name: String,
}

#[instrument(skip(state), name = "ipc::get_gateway_events")]
pub async fn get_gateway_events(
    State(state): State<IpcEndpointState>,
    Path(path_params): Path<PathParams>,
    Query(query_params): Query<QueryParams>,
) -> impl IntoResponse {
    if path_params.gateway_namespace.is_empty() {
        let mut problem = Problem::from(StatusCode::BAD_REQUEST)
            .with_value("status", StatusCode::BAD_GATEWAY.as_u16())
            .with_title("Invalid Namespace")
            .with_detail("Gateway namespace cannot be empty");

        if let Some(trace_id) = trace_id() {
            problem = problem.with_instance(trace_id);
        }

        return problem.into_response();
    }
    if path_params.gateway_name.is_empty() {
        let mut problem = Problem::from(StatusCode::BAD_REQUEST)
            .with_value("status", StatusCode::BAD_REQUEST.as_u16())
            .with_title("Invalid Name")
            .with_detail("Gateway name cannot be empty");

        if let Some(trace_id) = trace_id() {
            problem = problem.with_instance(trace_id);
        }

        return problem.into_response();
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
        let mut problem = Problem::from(StatusCode::NOT_FOUND)
            .with_value("status", StatusCode::NOT_FOUND.as_u16())
            .with_title("Gateway Not Found")
            .with_detail(format!("Object {gateway_ref} not found"));

        if let Some(trace_id) = trace_id() {
            problem = problem.with_instance(trace_id);
        }

        return problem.into_response();
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
