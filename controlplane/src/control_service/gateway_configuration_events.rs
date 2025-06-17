use crate::objects::ObjectRef;
use dashmap::{DashMap, Entry};
use getset::Getters;
use kubera_core::config::gateway::types::GatewayConfiguration;
use std::fmt::Display;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast::{channel as broadcast_channel, Sender as BroadcastSender};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

#[derive(Debug, Error)]
pub struct RecvError;

impl Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to receive event from broadcast channel")
    }
}

#[derive(Debug, Clone)]
pub enum GatewayConfigurationEventType {
    Update,
}

#[derive(Debug, Clone, Getters)]
pub struct GatewayConfigurationEvent {
    #[getset(get = "pub")]
    event_type: GatewayConfigurationEventType,
    #[getset(get = "pub")]
    gateway_ref: ObjectRef,
}

pub fn gateway_configuration_events_channel() -> (
    GatewayConfigurationEventSender,
    GatewayConfigurationEventStreamFactory,
) {
    let (tx, _) = broadcast_channel(128);
    (
        GatewayConfigurationEventSender {
            tx: tx.clone(),
            configurations: Arc::new(DashMap::new()),
        },
        GatewayConfigurationEventStreamFactory { tx },
    )
}

#[derive(Debug, Clone)]
pub struct GatewayConfigurationEventSender {
    tx: BroadcastSender<GatewayConfigurationEvent>,
    configurations: Arc<DashMap<ObjectRef, u64>>,
}

impl GatewayConfigurationEventSender {
    pub fn send(&self, gateway_ref: &ObjectRef, configuration: &GatewayConfiguration) -> () {
        let configuration_hash = {
            let mut hasher = DefaultHasher::new();
            configuration.hash(&mut hasher);
            hasher.finish()
        };
        let gateway_ref = gateway_ref.clone();
        match self.configurations.entry(gateway_ref.clone()) {
            Entry::Occupied(mut entry) => {
                if *entry.get() != configuration_hash {
                    entry.insert(configuration_hash);
                    let event = GatewayConfigurationEvent {
                        event_type: GatewayConfigurationEventType::Update,
                        gateway_ref: gateway_ref.clone(),
                    };
                    let _ = self.tx.send(event);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(configuration_hash);
                let event = GatewayConfigurationEvent {
                    event_type: GatewayConfigurationEventType::Update,
                    gateway_ref: gateway_ref.clone(),
                };
                let _ = self.tx.send(event);
            }
        }
    }
}

#[derive(Clone)]
pub struct GatewayConfigurationEventStreamFactory {
    tx: BroadcastSender<GatewayConfigurationEvent>,
}

impl GatewayConfigurationEventStreamFactory {
    pub fn for_gateway(
        &self,
        gateway_ref: ObjectRef,
    ) -> impl Stream<Item = Result<GatewayConfigurationEvent, RecvError>> + Send + use<> {
        let rx = self.tx.subscribe();

        BroadcastStream::new(rx).filter_map(move |event| match event {
            Ok(event) if event.gateway_ref == gateway_ref => Some(Ok(event)),
            Ok(_) => None,
            Err(_) => Some(Err(RecvError)),
        })
    }
}
