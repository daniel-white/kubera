pub mod endpoints;
pub mod events;
mod gateways;

use crate::ipc::endpoints::{
    SpawnIpcEndpointError, SpawnIpcEndpointParameters, SpawnIpcEndpointParametersBuilderError,
    spawn_ipc_endpoint,
};
use crate::ipc::events::EventSender;
use crate::ipc::gateways::{
    GatewayConfigurationManager, GatewayConfigurationManagerInsertError,
    create_gateway_configuration_services,
};
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::ObjectRef;
use crate::options::Options;
use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::ipc::{Event, GatewayEvent, Ref as IpcRef, RefBuilderError as IpcRefBuilderError};
use kubera_core::net::Port;
use kubera_core::sync::signal::Receiver;
use std::sync::Arc;
use thiserror::Error;
use tokio::task::JoinSet;

#[derive(Debug, Builder, Getters, CopyGetters)]
#[builder(setter(into))]
pub struct IpcServices {
    events: EventSender,

    gateway_configuration_manager: GatewayConfigurationManager,

    #[getset(get_copy = "pub")]
    port: Port,
}

#[derive(Debug, Error)]
pub enum TryFromObjectRefError {
    #[error("ObjectRef is missing a namespace")]
    MissingNamespace,
    #[error("Failed to create IpcRef from ObjectRef: {0}")]
    CreationError(#[from] IpcRefBuilderError),
}

impl TryFrom<ObjectRef> for IpcRef {
    type Error = TryFromObjectRefError;

    fn try_from(object_ref: ObjectRef) -> Result<Self, Self::Error> {
        (&object_ref).try_into()
    }
}

impl TryFrom<&ObjectRef> for IpcRef {
    type Error = TryFromObjectRefError;

    fn try_from(object_ref: &ObjectRef) -> Result<Self, Self::Error> {
        let namespace = object_ref
            .namespace()
            .clone()
            .ok_or(TryFromObjectRefError::MissingNamespace)?;

        let ipc_ref = IpcRef::new_builder()
            .namespace(namespace)
            .name(object_ref.name())
            .build()?;

        Ok(ipc_ref)
    }
}

#[derive(Debug, Error)]
#[error("Failed to insert gateway configuration")]
pub enum IpcInsertGatewayConfigurationError {
    Insert(#[from] GatewayConfigurationManagerInsertError),
    InvalidGatewayRef(#[from] TryFromObjectRefError),
}

impl IpcServices {
    fn new_builder() -> IpcServicesBuilder {
        IpcServicesBuilder::default()
    }

    pub fn try_insert_gateway_configuration(
        &self,
        gateway_ref: ObjectRef,
        configuration: GatewayConfiguration,
    ) -> Result<(), IpcInsertGatewayConfigurationError> {
        self.gateway_configuration_manager
            .try_insert(gateway_ref.clone(), configuration)?;

        let gateway_ref: IpcRef = gateway_ref.try_into()?;

        self.events
            .send(Event::Gateway(GatewayEvent::ConfigurationUpdate(
                gateway_ref,
            )));

        Ok(())
    }

    pub fn remove_gateway_configuration(&self, gateway_ref: &ObjectRef) {
        if self.gateway_configuration_manager.remove(gateway_ref)
            && let Ok(gateway_ref) = gateway_ref.try_into()
        {
            self.events
                .send(Event::Gateway(GatewayEvent::Deleted(gateway_ref)));
        }
    }
}

#[derive(Clone, Builder)]
#[builder(setter(into))]
pub struct SpawnIpcParameters {
    port: Port,
    kube_client: Receiver<Option<KubeClientCell>>,
    options: Arc<Options>,
}

impl SpawnIpcParameters {
    pub fn new_builder() -> SpawnIpcParametersBuilder {
        SpawnIpcParametersBuilder::default()
    }
}

#[derive(Debug, Error)]
pub enum SpawnIpcError {
    #[error("Failed to build IPC endpoint parameters")]
    EndpointParameters(#[from] SpawnIpcEndpointParametersBuilderError),
    #[error("Failed to create IPC services")]
    Services(#[from] IpcServicesBuilderError),
    #[error("Failed to spawn IPC endpoint")]
    SpawnEndpoint(#[from] SpawnIpcEndpointError),
}

pub async fn spawn_ipc(
    join_set: &mut JoinSet<()>,
    params: SpawnIpcParameters,
) -> Result<IpcServices, SpawnIpcError> {
    let (event_sender, events_factory) = events::events_channel();
    let (reader, gateway_manager) = create_gateway_configuration_services();

    let ipc_endpoint_params = SpawnIpcEndpointParameters::new_builder()
        .options(params.options)
        .port(params.port)
        .events(events_factory)
        .gateways(reader)
        .kube_client(params.kube_client)
        .build()?;

    spawn_ipc_endpoint(join_set, ipc_endpoint_params).await?;

    let ipc_services = IpcServices::new_builder()
        .events(event_sender)
        .gateway_configuration_manager(gateway_manager)
        .port(params.port)
        .build()?;

    Ok(ipc_services)
}
