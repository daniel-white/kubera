use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::{ObjectRef, ObjectUniqueId};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::{extract::State, response::IntoResponse};
use gateway_api::apis::standard::gateways::Gateway;
use problemdetails::Problem;
use serde::Deserialize;
use tracing::debug;

#[derive(Deserialize)]
pub struct PathParams {
    gateway_namespace: String,
    gateway_name: String,
    static_response_filter_id: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    pod_name: String,
}

pub async fn get_static_response(
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
    if path_params.static_response_filter_id.is_empty() {
        return Problem::from(StatusCode::BAD_REQUEST)
            .with_title("Invalid Static Response Filter ID")
            .with_detail("Static response filter ID cannot be empty")
            .into_response();
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
        return Problem::from(StatusCode::NOT_FOUND)
            .with_title("Gateway Not Found")
            .with_detail(format!("Gateway {gateway_ref} not found"))
            .into_response();
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
    Problem::from(StatusCode::NOT_FOUND)
        .with_title("Static Response Filter Not Found")
        .with_detail(format!(
            "Static response filter {} not found",
            path_params.static_response_filter_id
        ))
        .into_response()
}
