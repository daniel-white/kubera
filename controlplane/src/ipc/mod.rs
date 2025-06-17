pub mod endpoints;
pub mod gateway_events;

use self::endpoints::router;
use self::gateway_events::GatewayEventStreamFactory;
use crate::ipc::gateway_events::GatewayEventSender;
use anyhow::Result;
use axum::Router;
use derive_builder::Builder;
use getset::Getters;
use kubera_core::net::Port;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::{select, spawn};
use tracing::info;

#[derive(Builder, Getters, Clone)]
pub struct IpcServiceState {
    #[getset(get = "pub")]
    gateway_event_stream_factory: GatewayEventStreamFactory,
}

impl IpcServiceState {
    fn new_builder() -> IpcServiceStateBuilder {
        IpcServiceStateBuilder::default()
    }
}

#[derive(Debug, Builder, Getters)]
pub struct IpcConfiguration {
    port: Port,
}

impl IpcConfiguration {
    pub fn new_builder() -> IpcConfigurationBuilder {
        IpcConfigurationBuilder::default()
    }

    fn socket_address(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port.into()))
    }
}

#[derive(Debug, Builder, Getters)]
pub struct IpcServices {
    #[getset(get = "pub")]
    gateway_event_sender: GatewayEventSender,
}

impl IpcServices {
    fn new_builder() -> IpcServicesBuilder {
        IpcServicesBuilder::default()
    }
}

pub async fn spawn_ipc_service(ipc_configuration: IpcConfiguration) -> Result<IpcServices> {
    let (gateway_event_sender, gateway_event_stream_factory) =
        gateway_events::gateway_events_channel();

    let services = IpcServices::new_builder()
        .gateway_event_sender(gateway_event_sender)
        .build()?;

    let state = IpcServiceState::new_builder()
        .gateway_event_stream_factory(gateway_event_stream_factory)
        .build()?;

    let socket_address = ipc_configuration.socket_address();
    info!("Starting IPC service on {}", socket_address);

    let tcp_listener = TcpListener::bind(socket_address).await?;

    spawn(async move {
        select! {
            _ = axum::serve(tcp_listener, router(state)) => info!("IPC service stopped"),
            _ = tokio::signal::ctrl_c() => info!("Received shutdown signal, stopping IPC service")
        }
    });

    Ok(services)
}
