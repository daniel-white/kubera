use derive_builder::Builder;
use eventsource_client::{Client, ClientBuilder, SSE};
use futures::TryStreamExt;
use getset::Getters;
use kubera_core::sync::signal::Receiver;
use std::net::SocketAddr;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::task::JoinSet;
use tracing::info;
use url::Url;

#[derive(Clone, Debug, Getters, Builder)]
#[builder(setter(into))]
pub struct WatchGatewayEventsParams {
    primary_socket_addr: Receiver<Option<SocketAddr>>,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

impl WatchGatewayEventsParams {
    pub fn new_builder() -> WatchGatewayEventsParamsBuilder {
        WatchGatewayEventsParamsBuilder::default()
    }
}

pub fn watch_gateway_events(join_set: &mut JoinSet<()>, params: WatchGatewayEventsParams) {
    let primary_socket_addr = params.primary_socket_addr.clone();

    join_set.spawn(async move {
        'primary: loop {
            if let Some(socket_addr) = primary_socket_addr.current().as_ref() {
                let url = {
                    let mut events_url = Url::parse(&format!("http://{}", socket_addr))
                        .expect("Failed to parse URL");
                    events_url.set_path(&format!(
                        "/ipc/namespaces/{}/gateways/{}/events",
                        params.gateway_namespace, params.gateway_name
                    ));
                    events_url.set_query(Some(&format!("pod_name={}", params.pod_name)));
                    events_url
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
                        _ = primary_socket_addr.changed() => {
                            break 'events; // Restart the watcher if the socket address changes
                        },
                        event = event_stream.try_next() => {
                            match event {
                                Ok(Some(SSE::Event(event))) => {
                                    tracing::debug!("Received event: {:?}", event);
                                    // Process the event as needed
                                }
                                Ok(None) => {
                                    tracing::info!("Event stream ended");
                                    break 'events; // Exit if the stream ends
                                }
                                Err(e) => {
                                    tracing::error!("Error receiving event: {}", e);
                                    break 'events; // Exit on error
                                }
                                _ => {
                                    tracing::warn!("Received unexpected event type");
                                }
                            }
                        }
                    }
                }
            } else {
                info!("No primary socket address available, waiting for it to be set");
            }
        }
    });
}
