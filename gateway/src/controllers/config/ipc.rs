use http::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast::Receiver as BroadcastReceiver;
use tracing::{debug, info, warn};
use typed_builder::TypedBuilder;
use url::Url;
use vg_core::config::gateway::serde::read_configuration;
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::continue_on;
use vg_core::ipc::GatewayEvent;
use vg_core::sync::signal::{Receiver, Sender, signal};
use vg_core::task::Builder as TaskBuilder;

#[derive(Debug, TypedBuilder)]
pub struct FetchConfigurationParams {
    ipc_endpoint_rx: Receiver<SocketAddr>,
    gateway_events_rx: BroadcastReceiver<GatewayEvent>,
    #[builder(setter(into))]
    pod_name: String,
    #[builder(setter(into))]
    gateway_namespace: String,
    #[builder(setter(into))]
    gateway_name: String,
    client: Arc<ClientWithMiddleware>,
}

pub fn fetch_configuration(
    task_builder: &TaskBuilder,
    params: FetchConfigurationParams,
) -> Receiver<(Instant, GatewayConfiguration)> {
    let (tx, rx) = signal("fetched_configuration");

    task_builder
        .new_task(stringify!(fetch_configuration))
        .spawn(async move {
            let mut gateway_events = params.gateway_events_rx;
            loop {
                if let Some(ipc_endpoint_addr) = params.ipc_endpoint_rx.get().await
                    && let Ok(event) = gateway_events.recv().await
                    && let GatewayEvent::ConfigurationUpdate(_) = event
                {
                    let url = {
                        let mut url = Url::parse(&format!("http://{ipc_endpoint_addr}"))
                            .expect("Failed to parse URL");
                        url.set_path(&format!(
                            "/ipc/namespaces/{}/gateways/{}/configuration",
                            params.gateway_namespace, params.gateway_name
                        ));
                        url.set_query(Some(&format!("pod_name={}", params.pod_name)));
                        url
                    };

                    debug!("Fetching configuration from URL: {}", url);

                    let serial = Instant::now(); // capture the time before the request

                    match params.client.get(url).send().await {
                        Ok(response) if response.status() == StatusCode::OK => {
                            match response.bytes().await {
                                Ok(bytes) => {
                                    let buf = BufReader::new(bytes.as_ref());
                                    match read_configuration(buf) {
                                        Ok(configuration) => {
                                            debug!("Configuration fetched successfully");
                                            tx.set((serial, configuration)).await;
                                        }
                                        Err(err) => {
                                            warn!("Error reading configuration: {}", err);
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!("Error reading response body: {}", err);
                                }
                            }
                        }
                        Ok(response) => {
                            info!("Unexpected response fetching configuration: {:?}", response);
                        }
                        Err(err) => {
                            warn!("Error fetching configuration: {}", err);
                        }
                    }
                }
            }
        });

    rx
}

pub fn watch_ipc_endpoint(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
    tx: Sender<SocketAddr>,
) {
    let gateway_configuration_rx = gateway_configuration_rx.clone();

    task_builder
        .new_task(stringify!(watch_ipc_endpoint))
        .spawn(async move {
            loop {
                if let Some(gateway_configuration) = gateway_configuration_rx.get().await {
                    let primary_endpoint = gateway_configuration
                        .ipc()
                        .as_ref()
                        .and_then(|c| *c.endpoint());

                    tx.replace(primary_endpoint).await;
                }

                continue_on!(gateway_configuration_rx.changed());
            }
        });
}
