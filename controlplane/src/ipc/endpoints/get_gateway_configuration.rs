use crate::ipc::IpcServiceState;
use crate::objects::ObjectRef;
use axum::extract::{Path, Query};
use axum::{extract::State, response::IntoResponse};
use gateway_api::apis::standard::gateways::Gateway;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PathParams {
    gateway_namespace: String,
    gateway_name: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    pod_name: String,
}

pub async fn get_gateway_configuration(
    State(_state): State<IpcServiceState>,
    Path(path_params): Path<PathParams>,
    Path(query_params): Query<PathParams>,
) -> impl IntoResponse {
    let _gateway_ref = ObjectRef::new_builder()
        .of_kind::<Gateway>()
        .name(path_params.gateway_name)
        .namespace(Some(path_params.gateway_namespace))
        .build()
        .unwrap();

    axum::http::StatusCode::NOT_FOUND
}
