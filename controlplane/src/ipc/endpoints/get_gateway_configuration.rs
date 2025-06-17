use crate::ipc::IpcServiceState;
use crate::objects::ObjectRef;
use axum::extract::Path;
use axum::{
    extract::State,
    response::IntoResponse,
};
use gateway_api::apis::standard::gateways::Gateway;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetGatewayConfigurationPathParams {
    namespace: String,
    name: String,
}

pub async fn get_gateway_configuration(
    State(state): State<IpcServiceState>,
    Path(params): Path<GetGatewayConfigurationPathParams>,
) -> impl IntoResponse {
    let gateway_ref = ObjectRef::new_builder()
        .of_kind::<Gateway>()
        .name(params.name)
        .namespace(Some(params.namespace))
        .build()
        .unwrap();

    axum::http::StatusCode::NOT_FOUND
}
