use crate::objects::ObjectRef;
use kubera_core::ipc::{Event, GatewayEvent};
use std::fmt::Display;
use std::hash::Hash;
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

pub fn events_channel() -> (EventSender, EventStreamFactory) {
    let (tx, _) = broadcast_channel(128);
    (EventSender { tx: tx.clone() }, EventStreamFactory { tx })
}

#[derive(Debug, Clone)]
pub struct EventSender {
    tx: BroadcastSender<Event>,
}

impl EventSender {
    pub fn send(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}

#[derive(Clone)]
pub struct EventStreamFactory {
    tx: BroadcastSender<Event>,
}

impl EventStreamFactory {
    pub fn all_events(&self) -> impl Stream<Item = Result<Event, RecvError>> + Send + use<> {
        BroadcastStream::new(self.tx.subscribe()).filter_map(|event| match event {
            Ok(event) => Some(Ok(event)),
            Err(_) => Some(Err(RecvError)),
        })
    }

    pub fn gateway_events(
        &self,
    ) -> impl Stream<Item = Result<GatewayEvent, RecvError>> + Send + use<> {
        self.all_events().filter_map(|event| match event {
            Ok(Event::Gateway(event)) => Some(Ok(event)),
            Ok(_) => None,
            Err(_) => Some(Err(RecvError)),
        })
    }

    pub fn named_gateway_events(
        &self,
        gateway_ref: ObjectRef,
    ) -> impl Stream<Item = Result<GatewayEvent, RecvError>> + Send + use<> {
        self.gateway_events().filter(move |event| match event {
            Ok(GatewayEvent::ConfigurationUpdate { name, namespace }) => {
                name == gateway_ref.name() && Some(namespace) == gateway_ref.namespace().as_ref()
            }
            _ => false,
        })
    }
}
