use eventsource_client::{Client, ClientBuilder, SSE};
use futures::TryStreamExt;
use getset::Getters;
use reqwest_middleware::ClientWithMiddleware;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::broadcast::{Sender, channel};
use tracing::{debug, info};
use typed_builder::TypedBuilder;
use url::Url;
use vg_core::continue_on;
use vg_core::ipc::GatewayEvent;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;

#[derive(Debug, Getters, TypedBuilder)]
pub struct PollGatewayEventsParams {
    ipc_endpoint_rx: Receiver<SocketAddr>,
    #[builder(setter(into))]
    pod_name: String,
    #[builder(setter(into))]
    gateway_namespace: String,
    #[builder(setter(into))]
    gateway_name: String,
    client: Arc<ClientWithMiddleware>,
}

pub fn poll_gateway_events(
    task_builder: &TaskBuilder,
    params: PollGatewayEventsParams,
) -> Sender<GatewayEvent> {
    let (tx, _) = channel(20);
    let ipc_endpoint_rx = params.ipc_endpoint_rx.clone();
    let events_tx = tx.clone();

    task_builder
        .new_task(stringify!(poll_gateway_events))
        .spawn(async move {
            'primary: loop {
                if let Some(ipc_endpoint_addr) = ipc_endpoint_rx.get().await {
                    let url = {
                        let mut url = Url::parse(&format!("http://{ipc_endpoint_addr}"))
                            .expect("Failed to parse URL");
                        url.set_path(&format!(
                            "/ipc/namespaces/{}/gateways/{}/events",
                            params.gateway_namespace, params.gateway_name
                        ));
                        url.set_query(Some(&format!("pod_name={}", params.pod_name)));
                        url
                    };

                    info!("Watching events at URL: {}", url);

                    let client = ClientBuilder::for_url(url.as_str())
                        .expect("Failed to create event source client builder")
                        .build();

                    let mut event_stream = Box::pin(client.stream());

                    'events: loop {
                        select! {
                        _ = ctrl_c() => {
                            break 'primary; // Exit the loop, shutting down the watcher
                        },
                        _ = ipc_endpoint_rx.changed() => {
                            break 'events; // Restart the watcher if the socket address changes
                        },
                        event = event_stream.try_next() => {
                            match event {
                                Ok(Some(SSE::Event(event))) => {
                                    let _ = GatewayEvent::try_parse(event.event_type, event.data)
                                        .map(|gateway_event| {
                                            tracing::debug!("Received gateway event: {:?}", gateway_event);
                                            events_tx.send(gateway_event).ok();
                                        })
                                        .inspect_err(|e| {
                                            tracing::error!("Failed to parse gateway event: {}", e);
                                        });
                                }
                                Ok(Some(_)) => {
                                    tracing::debug!("Received non-event SSE message");
                                }
                                Ok(None) => {
                                    tracing::info!("Event stream ended");
                                    break 'events; // Exit if the stream ends
                                }
                                Err(e) => {
                                    tracing::error!("Error receiving event: {}", e);
                                    break 'events; // Exit on error
                                }
                            }
                        }
                    }
                    }
                } else {
                    continue_on!(ipc_endpoint_rx.changed());
                }
            }
        });

    tx
}
