use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::ObjectRef;
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
}

#[derive(Deserialize, Debug)]
pub struct QueryParams {
    pod_name: String,
}

#[instrument(skip(state), name = "ipc::get_gateway_configuration")]
pub async fn get_gateway_configuration(
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
        "Pod {} requesting gateway configuration for {:?}",
        query_params.pod_name, gateway_ref
    );

    if let Some(config) = state.gateways.get_configuration_yaml(&gateway_ref) {
        debug!("Returning configuration for {}", gateway_ref);
        (
            StatusCode::OK,
            [(CONTENT_TYPE, "application/yaml")],
            config.value().clone(),
        )
            .into_response()
    } else {
        debug!("Configuration for {} not found", gateway_ref);
        let mut problem = Problem::from(StatusCode::NOT_FOUND)
            .with_value("status", StatusCode::NOT_FOUND.as_u16())
            .with_title("Gateway Configuration Not Found")
            .with_detail(format!("Configuration for object {gateway_ref} not found"));

        if let Some(trace_id) = trace_id() {
            problem = problem.with_instance(trace_id);
        }

        problem.into_response()
    }
}
