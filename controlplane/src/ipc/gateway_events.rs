use crate::objects::ObjectRef;
use dashmap::{DashMap, Entry};
use derive_builder::Builder;
use getset::Getters;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::ipc::GatewayEventType;
use std::fmt::Display;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast::{Sender as BroadcastSender, channel as broadcast_channel};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

#[derive(Debug, Error)]
pub struct RecvError;

impl Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to receive event from broadcast channel")
    }
}

#[derive(Debug, Builder, Clone, Getters)]
pub struct GatewayEvent {
    #[getset(get = "pub")]
    gateway_ref: ObjectRef,

    #[getset(get = "pub")]
    event_type: GatewayEventType,
}

pub fn gateway_events_channel() -> (GatewayEventSender, GatewayEventStreamFactory) {
    let (tx, _) = broadcast_channel(128);
    (
        GatewayEventSender {
            tx: tx.clone(),
            configuration_hashes: Arc::new(DashMap::new()),
        },
        GatewayEventStreamFactory { tx },
    )
}

#[derive(Debug, Clone)]
pub struct GatewayEventSender {
    tx: BroadcastSender<GatewayEvent>,
    configuration_hashes: Arc<DashMap<ObjectRef, u64>>,
}

impl GatewayEventSender {
    pub fn on_configuration_update(
        &self,
        gateway_ref: &ObjectRef,
        configuration: &GatewayConfiguration,
    ) {
        let configuration_hash = {
            let mut hasher = DefaultHasher::new();
            configuration.hash(&mut hasher);
            hasher.finish()
        };
        let gateway_ref = gateway_ref.clone();
        match self.configuration_hashes.entry(gateway_ref.clone()) {
            Entry::Occupied(mut entry) => {
                if *entry.get() != configuration_hash {
                    entry.insert(configuration_hash);
                    self.send_configuration_update_event(gateway_ref);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(configuration_hash);
                self.send_configuration_update_event(gateway_ref);
            }
        };
    }

    fn send_configuration_update_event(&self, gateway_ref: ObjectRef) {
        let event = GatewayEventBuilder::default()
            .gateway_ref(gateway_ref)
            .event_type(GatewayEventType::ConfigurationUpdated)
            .build()
            .expect("Failed to build GatewayEvent");
        let _ = self.tx.send(event);
    }
}

#[derive(Clone)]
pub struct GatewayEventStreamFactory {
    tx: BroadcastSender<GatewayEvent>,
}

impl GatewayEventStreamFactory {
    pub fn for_gateway(
        &self,
        gateway_ref: ObjectRef,
    ) -> impl Stream<Item = Result<GatewayEvent, RecvError>> + Send + use<> {
        let rx = self.tx.subscribe();

        BroadcastStream::new(rx).filter_map(move |event| match event {
            Ok(event) if event.gateway_ref == gateway_ref => Some(Ok(event)),
            Ok(_) => None,
            Err(_) => Some(Err(RecvError)),
        })
    }
}
