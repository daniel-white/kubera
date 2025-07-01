pub mod endpoints;
pub mod events;
mod gateways;

use self::endpoints::router;
use self::events::EventStreamFactory;
use crate::ipc::events::EventSender;
use crate::ipc::gateways::{
    GatewayConfigurationManager, GatewayConfigurationReader, create_gateway_configuration_services,
};
use crate::objects::ObjectRef;
use anyhow::Result;
use derive_builder::Builder;
use getset::Getters;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::ipc::{Event, GatewayEvent, Ref as IpcRef};
use kubera_core::net::Port;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::{select, spawn};
use tracing::info;

#[derive(Builder, Getters, Clone)]
pub struct IpcServicesState {
    #[getset(get = "pub")]
    events: EventStreamFactory,

    #[getset(get = "pub")]
    gateways: GatewayConfigurationReader,
}

impl IpcServicesState {
    fn new_builder() -> IpcServicesStateBuilder {
        IpcServicesStateBuilder::default()
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

    fn endpoint(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port.into()))
    }
}

#[derive(Debug, Builder, Getters)]
#[builder(setter(into))]
pub struct IpcServices {
    events: EventSender,

    gateway_configuration_manager: GatewayConfigurationManager,

    #[getset(get = "pub")]
    port: Port,
}

impl From<ObjectRef> for IpcRef {
    fn from(object_ref: ObjectRef) -> Self {
        (&object_ref).into()
    }
}

impl From<&ObjectRef> for IpcRef {
    fn from(object_ref: &ObjectRef) -> Self {
        IpcRef::new_builder()
            .namespace(
                object_ref
                    .namespace()
                    .clone()
                    .expect("ObjectRef must have a namespace"),
            )
            .name(object_ref.name())
            .build()
            .expect("Failed to create IpcRef from ObjectRef")
    }
}

impl IpcServices {
    fn new_builder() -> IpcServicesBuilder {
        IpcServicesBuilder::default()
    }

    pub fn insert_gateway_configuration(
        &self,
        gateway_ref: ObjectRef,
        configuration: &GatewayConfiguration,
    ) {
        self.gateway_configuration_manager
            .insert(gateway_ref.clone(), configuration);
        self.events
            .send(Event::Gateway(GatewayEvent::ConfigurationUpdate(
                gateway_ref.into(),
            )));
    }

    pub fn remove_gateway_configuration(&self, gateway_ref: &ObjectRef) {
        self.gateway_configuration_manager.remove(gateway_ref);
        self.events
            .send(Event::Gateway(GatewayEvent::Deleted(gateway_ref.into())));
    }
}

pub async fn spawn_ipc_service(ipc_configuration: IpcServiceConfiguration) -> Result<IpcServices> {
    let (event_sender, events_factory) = events::events_channel();
    let endpoint = ipc_configuration.endpoint();

    let (reader, gateway_manager) = create_gateway_configuration_services();

    let services = IpcServices::new_builder()
        .events(event_sender)
        .gateway_configuration_manager(gateway_manager)
        .port(Port::new(endpoint.port()))
        .build()?;

    let state = IpcServicesState::new_builder()
        .gateways(reader)
        .events(events_factory)
        .build()?;

    info!("Starting IPC service on {}", endpoint);

    let tcp_listener = TcpListener::bind(endpoint).await?;

    spawn(async move {
        select! {
            _ = axum::serve(tcp_listener, router(state)) => info!("IPC service stopped"),
            _ = tokio::signal::ctrl_c() => info!("Received shutdown signal, stopping IPC service")
        }
    });

    Ok(services)
}
