mod get_gateway_configuration;
mod get_gateway_events;
mod liveness_check;

use self::get_gateway_configuration::get_gateway_configuration;
use self::get_gateway_events::get_gateway_events;
use crate::ipc::events::EventStreamFactory;
use crate::ipc::gateways::GatewayConfigurationReader;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use derive_builder::Builder;
use getset::{CloneGetters, CopyGetters, Getters};
use kubera_core::net::Port;
use problemdetails::Problem;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::select;
use tokio::task::JoinSet;
use tracing::info;

#[derive(Builder, Getters, Clone)]
pub(self) struct IpcEndpointState {
    #[getset(get = "pub")]
    events: EventStreamFactory,

    #[getset(get = "pub")]
    gateways: GatewayConfigurationReader,
}

impl IpcEndpointState {
    fn new_builder() -> IpcEndpointStateBuilder {
        IpcEndpointStateBuilder::default()
    }
}

#[derive(Debug, Builder, CloneGetters, CopyGetters)]
#[builder(setter(into))]
pub(super) struct SpawnIpcEndpointParameters {
    #[getset(get_copy = "")]
    port: Port,

    #[getset(get_clone = "")]
    events: EventStreamFactory,

    #[getset(get_clone = "")]
    gateways: GatewayConfigurationReader,
}

impl SpawnIpcEndpointParameters {
    pub fn new_builder() -> SpawnIpcEndpointParametersBuilder {
        SpawnIpcEndpointParametersBuilder::default()
    }

    fn endpoint(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port().into()))
    }
}

pub(super) fn spawn_ipc_endpoint(join_set: &mut JoinSet<()>, params: SpawnIpcEndpointParameters) {
    join_set.spawn(async move {
        let initial_state = IpcEndpointState::new_builder()
            .gateways(params.gateways())
            .events(params.events())
            .build()
            .expect("Failed to create initial IPC endpoint state");

        let endpoint = params.endpoint();
        let tcp_listener = TcpListener::bind(endpoint)
            .await
            .expect("Failed to bind IPC endpoint");
        select! {
            _ = axum::serve(tcp_listener, router(initial_state)) => info!("IPC service stopped"),
            _ = tokio::signal::ctrl_c() => info!("Received shutdown signal, stopping IPC service")
        }
    });
}

fn router(state: IpcEndpointState) -> Router {
    Router::new()
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/configuration",
            get(get_gateway_configuration),
        )
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/events",
            get(get_gateway_events),
        )
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(state)
}

async fn not_found() -> impl IntoResponse {
    Problem::from(StatusCode::NOT_FOUND)
        .with_title("Not Found")
        .with_detail("The requested resource could not be found")
}

async fn method_not_allowed() -> impl IntoResponse {
    Problem::from(StatusCode::METHOD_NOT_ALLOWED)
        .with_title("Method Not Allowed")
        .with_detail("The requested method is not allowed for this resource")
}
