pub mod endpoints;
pub mod events;

use self::endpoints::router;
use self::events::EventStreamFactory;
use crate::ipc::events::EventSender;
use anyhow::Result;
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
    events: EventStreamFactory,
}

impl IpcServiceState {
    fn new_builder() -> IpcServiceStateBuilder {
        IpcServiceStateBuilder::default()
    }
}

#[derive(Debug, Builder, Getters)]
pub struct IpcServiceConfiguration {
    port: Port,
}

impl IpcServiceConfiguration {
    pub fn new_builder() -> IpcServiceConfigurationBuilder {
        IpcServiceConfigurationBuilder::default()
    }

    fn socket_address(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port.into()))
    }
}

#[derive(Debug, Builder, Getters)]
pub struct IpcServices {
    #[getset(get = "pub")]
    events: EventSender,
}

impl IpcServices {
    fn new_builder() -> IpcServicesBuilder {
        IpcServicesBuilder::default()
    }
}

pub async fn spawn_ipc_service(ipc_configuration: IpcServiceConfiguration) -> Result<IpcServices> {
    let (event_sender, factory) = events::events_channel();

    let services = IpcServices::new_builder().events(event_sender).build()?;

    let state = IpcServiceState::new_builder().events(factory).build()?;

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
