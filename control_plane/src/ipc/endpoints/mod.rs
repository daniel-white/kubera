mod get_gateway_configuration;
mod get_gateway_events;
mod liveness_check;

use self::get_gateway_configuration::get_gateway_configuration;
use self::get_gateway_events::get_gateway_events;
use crate::health::KubernetesApiHealthIndicator;
use crate::ipc::endpoints::liveness_check::liveness_check;
use crate::ipc::events::EventStreamFactory;
use crate::ipc::gateways::GatewayConfigurationReader;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum_health::Health;
use derive_builder::Builder;
use getset::{CloneGetters, CopyGetters, Getters};
use kubera_core::net::Port;
use kubera_core::sync::signal::Receiver;
use kubera_core::task::Builder as TaskBuilder;
use problemdetails::Problem;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::select;
use tracing::info;

#[derive(Builder, Getters, CloneGetters, Clone)]
pub struct IpcEndpointState {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,

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

#[derive(Builder, CloneGetters, CopyGetters)]
#[builder(setter(into))]
pub struct SpawnIpcEndpointParameters {
    #[getset(get_clone = "")]
    options: Arc<Options>,

    #[getset(get_copy = "")]
    port: Port,

    #[getset(get_clone = "")]
    events: EventStreamFactory,

    #[getset(get_clone = "")]
    gateways: GatewayConfigurationReader,

    #[getset(get_clone = "")]
    kube_client_rx: Receiver<KubeClientCell>,
}

impl SpawnIpcEndpointParameters {
    pub fn new_builder() -> SpawnIpcEndpointParametersBuilder {
        SpawnIpcEndpointParametersBuilder::default()
    }

    fn endpoint(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port().into()))
    }
}

#[derive(Debug, Error)]
pub enum SpawnIpcEndpointError {
    #[error("Failed to create initial IPC endpoint state: {0}")]
    InitialState(#[from] IpcEndpointStateBuilderError),

    #[error("Failed to bind IPC endpoint: {0}")]
    NetworkBind(#[from] std::io::Error),
}

pub async fn spawn_ipc_endpoint(
    task_builder: &TaskBuilder,
    params: SpawnIpcEndpointParameters,
) -> Result<(), SpawnIpcEndpointError> {
    let initial_state = IpcEndpointState::new_builder()
        .options(params.options())
        .gateways(params.gateways())
        .events(params.events())
        .build()?;

    let kube_health = KubernetesApiHealthIndicator::new(&params.kube_client_rx);
    let health = Health::builder().with_indicator(kube_health).build();

    let endpoint = params.endpoint();
    let tcp_listener = TcpListener::bind(endpoint).await?;

    task_builder
        .new_task("ipc_endpoint")
        .spawn(async move {
            select! {
                _ = axum::serve(tcp_listener, router(initial_state, health)) => info!("IPC service stopped"),
                _ = tokio::signal::ctrl_c() => info!("Received shutdown signal, stopping IPC service")
            }
        });

    Ok(())
}

fn router(state: IpcEndpointState, health: Health) -> Router {
    Router::new()
        .route("/healthz/liveness", get(liveness_check))
        .route("/healthz/readiness", get(axum_health::health))
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
        .layer(health)
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
