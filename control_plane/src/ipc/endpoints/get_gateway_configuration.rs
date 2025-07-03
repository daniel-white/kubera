use crate::ipc::endpoints::IpcEndpointState;
use crate::kubernetes::objects::ObjectRef;
use axum::extract::{Path, Query};
use axum::{extract::State, response::IntoResponse};
use gateway_api::apis::standard::gateways::Gateway;
use problemdetails::Problem;
use serde::Deserialize;
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

pub async fn get_gateway_configuration(
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
        "Pod {} requesting gateway configuration for {}",
        query_params.pod_name, gateway_ref
    );

    match state.gateways.get_configuration_yaml(&gateway_ref) {
        Some(config) => {
            debug!("Returning configuration for {}", gateway_ref);
            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/yaml")],
                config.value().clone(),
            )
                .into_response()
        }
        None => {
            debug!("Configuration for {} not found", gateway_ref);
            Problem::from(axum::http::StatusCode::NOT_FOUND)
                .with_title("Gateway Configuration Not Found")
                .with_detail(format!(
                    "Configuration for object {gateway_ref} not found"
                ))
                .into_response()
        }
    }
}
