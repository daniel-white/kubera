use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::{ObjectRef, ObjectUniqueId};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::{extract::State, response::IntoResponse};
use gateway_api::apis::standard::gateways::Gateway;
use problemdetails::Problem;
use serde::Deserialize;
use tracing::{debug, instrument};
use vg_core::instrumentation::trace_id;

#[derive(Deserialize, Debug)]
pub struct PathParams {
    gateway_namespace: String,
    gateway_name: String,
    static_response_filter_id: String,
}

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pod_name: String,
}

#[instrument(skip(state), name = "ipc::get_static_response")]
pub async fn get_static_response(
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
    if path_params.static_response_filter_id.is_empty() {
        let mut problem = Problem::from(StatusCode::BAD_REQUEST)
            .with_value("status", StatusCode::BAD_REQUEST.as_u16())
            .with_title("Invalid Static Response Filter ID")
            .with_detail("Static response filter ID cannot be empty");

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
        "Pod {} requesting static response for {:?} with filter ID {}",
        query_params.pod_name, gateway_ref, path_params.static_response_filter_id
    );

    if !state.gateways.exists(&gateway_ref) {
        debug!("Gateway for {} not found", gateway_ref);
        let mut problem = Problem::from(StatusCode::NOT_FOUND)
            .with_value("status", StatusCode::NOT_FOUND.as_u16())
            .with_title("Gateway Not Found")
            .with_detail(format!("Gateway {gateway_ref} not found"));

        if let Some(trace_id) = trace_id() {
            problem = problem.with_instance(trace_id);
        }

        return problem.into_response();
    }

    if let Some((content_type, body)) = state
        .static_responses_cache()
        .get(ObjectUniqueId::new(&path_params.static_response_filter_id))
        .await
    {
        return (StatusCode::OK, [(CONTENT_TYPE, content_type)], body).into_response();
    }

    debug!(
        "Static response filter {} not found",
        path_params.static_response_filter_id
    );
    let mut problem = Problem::from(StatusCode::NOT_FOUND)
        .with_value("status", StatusCode::NOT_FOUND.as_u16())
        .with_title("Static Response Filter Not Found")
        .with_detail(format!(
            "Static response filter {} not found",
            path_params.static_response_filter_id
        ));

    if let Some(trace_id) = trace_id() {
        problem = problem.with_instance(trace_id);
    }

    problem.into_response()
}
