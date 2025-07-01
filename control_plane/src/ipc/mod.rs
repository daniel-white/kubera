pub mod endpoints;
pub mod events;
mod gateways;

use crate::ipc::endpoints::{SpawnIpcEndpointParameters, spawn_ipc_endpoint};
use crate::ipc::events::EventSender;
use crate::ipc::gateways::{GatewayConfigurationManager, create_gateway_configuration_services};
use crate::kubernetes::objects::ObjectRef;
use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::ipc::{Event, GatewayEvent, Ref as IpcRef};
use kubera_core::net::Port;
use tokio::task::JoinSet;

#[derive(Debug, Builder, Getters, CopyGetters)]
#[builder(setter(into))]
pub struct IpcServices {
    events: EventSender,

    gateway_configuration_manager: GatewayConfigurationManager,
    
    #[getset(get_copy = "pub")]
    port: Port
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

#[derive(Debug, Clone, Builder)]
#[builder(setter(into))]
pub struct SpawnIpcParameters {
    port: Port,
}

impl SpawnIpcParameters {
    pub fn new_builder() -> SpawnIpcParametersBuilder {
        SpawnIpcParametersBuilder::default()
    }
}

pub fn spawn_ipc(join_set: &mut JoinSet<()>, params: SpawnIpcParameters) -> IpcServices {
    let (event_sender, events_factory) = events::events_channel();
    let (reader, gateway_manager) = create_gateway_configuration_services();

    let ipc_endpoint_params = SpawnIpcEndpointParameters::new_builder()
        .port(params.port)
        .events(events_factory)
        .gateways(reader)
        .build()
        .expect("Failed to build IPC endpoint parameters");

    spawn_ipc_endpoint(join_set, ipc_endpoint_params);

    IpcServices::new_builder()
        .events(event_sender)
        .gateway_configuration_manager(gateway_manager)
        .build()
        .expect("Failed to build IpcServices")
}
