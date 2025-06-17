use crate::control_service::gateway_configuration_events::{
    GatewayConfigurationEventStreamFactory, GatewayConfigurationEventType,
};
use crate::objects::ObjectRef;
use axum::extract::Path;
use axum::response::sse::Event;
use axum::{
    extract::State,
    response::{sse::Sse, IntoResponse},
};
use futures::TryStreamExt;
use gateway_api::apis::standard::gateways::Gateway;
use kubera_core::ipc::GatewayEvent;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

#[derive(Deserialize)]
pub struct GatewayReferenceParams {
    namespace: String,
    name: String,
}

pub async fn stream_gateway_configuration_events(
    State(state): State<Arc<GatewayConfigurationEventStreamFactory>>,
    Path(params): Path<GatewayReferenceParams>,
) -> impl IntoResponse {
    let gateway_ref = ObjectRef::new_builder()
        .of_kind::<Gateway>()
        .name(params.name)
        .namespace(Some(params.namespace))
        .build()
        .unwrap();

    let stream = state.for_gateway(gateway_ref);
    let stream = stream.map_ok(|e| match e.event_type() {
        GatewayConfigurationEventType::Update => {
            Event::default().event(GatewayEvent::ConfigurationUpdated)
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
