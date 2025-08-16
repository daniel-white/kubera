use crate::kubernetes::objects::ObjectRef;
use std::fmt::Display;
use std::sync::LazyLock;
use thiserror::Error;
use tokio::sync::broadcast::{channel as broadcast_channel, Sender as BroadcastSender};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use vg_core::instrumentation::KeyValues;
use vg_core::ipc::{Event, GatewayEvent};

#[derive(Debug, Error)]
pub struct RecvError;

impl Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to receive event from broadcast channel")
    }
}

pub fn events_channel() -> (EventSender, EventStreamFactory) {
    let (tx, _) = broadcast_channel(20);
    (EventSender { tx: tx.clone() }, EventStreamFactory { tx })
}

#[derive(Debug, Clone)]
pub struct EventSender {
    tx: BroadcastSender<Event>,
}

impl EventSender {
    pub fn send(&self, event: Event) {
        record_event(&event);
        let _ = self.tx.send(event);
    }
}

#[derive(Clone, Debug)]
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
            Ok(GatewayEvent::ConfigurationUpdate(ref_)) => {
                ref_.name() == gateway_ref.name()
                    && Some(ref_.namespace()) == gateway_ref.namespace().as_ref()
            }
            _ => false,
        })
    }
}

fn record_event(event: &Event) {
    use crate::instrumentation::METER;
    use opentelemetry::metrics::Counter;

    static COUNTER: LazyLock<Counter<u64>> = LazyLock::new(|| {
        METER
            .u64_counter("vg_control_plane_events")
            .with_description("Indicates the number of control plane events")
            .build()
    });

    COUNTER.add(1, &event.key_values());
}
