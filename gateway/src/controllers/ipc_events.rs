use eventsource_client::{Client, ClientBuilder, SSE};
use futures::TryStreamExt;
use getset::Getters;
use kubera_core::continue_on;
use kubera_core::ipc::GatewayEvent;
use kubera_core::sync::signal::Receiver;
use std::net::SocketAddr;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::broadcast::{Sender, channel};
use tokio::task::JoinSet;
use tracing::info;
use url::Url;

#[derive(Debug, Getters)]
pub struct PollGatewayEventsParams {
    ipc_endpoint: Receiver<Option<SocketAddr>>,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

impl PollGatewayEventsParams {
    pub fn new_builder() -> PollGatewayEventsParamsBuilder {
        PollGatewayEventsParamsBuilder::default()
    }
}

#[derive(Default)]
pub struct PollGatewayEventsParamsBuilder {
    ipc_endpoint: Option<Receiver<Option<SocketAddr>>>,
    pod_name: Option<String>,
    gateway_namespace: Option<String>,
    gateway_name: Option<String>,
}

impl PollGatewayEventsParamsBuilder {
    pub fn ipc_endpoint(&mut self, addr: &Receiver<Option<SocketAddr>>) -> &mut Self {
        self.ipc_endpoint = Some(addr.clone());
        self
    }

    pub fn pod_name<N: AsRef<str>>(&mut self, name: N) -> &mut Self {
        self.pod_name = Some(name.as_ref().to_string());
        self
    }

    pub fn gateway_namespace<N: AsRef<str>>(&mut self, namespace: N) -> &mut Self {
        self.gateway_namespace = Some(namespace.as_ref().to_string());
        self
    }

    pub fn gateway_name<N: AsRef<str>>(&mut self, name: N) -> &mut Self {
        self.gateway_name = Some(name.as_ref().to_string());
        self
    }

    pub fn build(self) -> PollGatewayEventsParams {
        PollGatewayEventsParams {
            ipc_endpoint: self
                .ipc_endpoint
                .expect("Primary socket address is required"),
            pod_name: self.pod_name.expect("Pod name is required"),
            gateway_namespace: self
                .gateway_namespace
                .expect("Gateway namespace is required"),
            gateway_name: self.gateway_name.expect("Gateway name is required"),
        }
    }
}

pub fn poll_gateway_events(
    join_set: &mut JoinSet<()>,
    params: PollGatewayEventsParams,
) -> Sender<GatewayEvent> {
    let (tx, _) = channel(20);

    let ipc_endpoint = params.ipc_endpoint.clone();

    let events_tx = tx.clone();
    join_set.spawn(async move {
        'primary: loop {
            if let Some(ipc_endpoint_addr) = ipc_endpoint.current().as_ref() {
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
                        _ = ipc_endpoint.changed() => {
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
                continue_on!(ipc_endpoint.changed());
            }
        }
    });

    tx
}
