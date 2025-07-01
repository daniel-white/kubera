use http::StatusCode;
use kubera_core::config::gateway::serde::read_configuration;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::continue_on;
use kubera_core::ipc::GatewayEvent;
use kubera_core::sync::signal::{Receiver, Sender, channel};
use reqwest::Client;
use std::io::BufReader;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::sync::broadcast::Receiver as BroadcastReceiver;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};
use url::Url;

#[derive(Debug)]
pub struct FetchConfigurationParams {
    ipc_endpoint: Receiver<Option<SocketAddr>>,
    gateway_events: BroadcastReceiver<GatewayEvent>,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

impl FetchConfigurationParams {
    pub fn new_builder() -> FetchConfigurationParamsBuilder {
        FetchConfigurationParamsBuilder::default()
    }
}

#[derive(Default)]
pub struct FetchConfigurationParamsBuilder {
    ipc_endpoint: Option<Receiver<Option<SocketAddr>>>,
    gateway_events: Option<BroadcastReceiver<GatewayEvent>>,
    pod_name: Option<String>,
    gateway_namespace: Option<String>,
    gateway_name: Option<String>,
}

impl FetchConfigurationParamsBuilder {
    pub fn ipc_endpoint(&mut self, addr: &Receiver<Option<SocketAddr>>) -> &mut Self {
        self.ipc_endpoint = Some(addr.clone());
        self
    }

    pub fn gateway_events(&mut self, events: BroadcastReceiver<GatewayEvent>) -> &mut Self {
        self.gateway_events = Some(events);
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

    pub fn build(self) -> FetchConfigurationParams {
        FetchConfigurationParams {
            ipc_endpoint: self
                .ipc_endpoint
                .expect("Primary socket address is required"),
            gateway_events: self
                .gateway_events
                .expect("Gateway events receiver is required"),
            pod_name: self.pod_name.expect("Pod name is required"),
            gateway_namespace: self
                .gateway_namespace
                .expect("Gateway namespace is required"),
            gateway_name: self.gateway_name.expect("Gateway name is required"),
        }
    }
}

pub fn fetch_configuration(
    join_set: &mut JoinSet<()>,
    params: FetchConfigurationParams,
) -> Receiver<Option<(Instant, GatewayConfiguration)>> {
    let (tx, rx) = channel(None);

    join_set.spawn(async move {
        let mut gateway_events = params.gateway_events;
        let client = Client::new();
        loop {
            if let Some(ipc_endpoint_addr) = params.ipc_endpoint.current().as_ref()
                && let Ok(event) = gateway_events.recv().await
                && let GatewayEvent::ConfigurationUpdate(_) = event
            {
                let url = {
                    let mut url = Url::parse(&format!("http://{}", ipc_endpoint_addr))
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

                match client.get(url).send().await {
                    Ok(response) if response.status() == StatusCode::OK => {
                        match response.bytes().await {
                            Ok(bytes) => {
                                let buf = BufReader::new(bytes.as_ref());
                                match read_configuration(buf) {
                                    Ok(configuration) => {
                                        debug!("Configuration fetched successfully");
                                        tx.replace(Some((serial, configuration)));
                                    }
                                    Err(err) => {
                                        warn!("Error reading configuration: {}", err);
                                        tx.replace(None);
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Error reading response body: {}", err);
                                tx.replace(None);
                            }
                        }
                    }
                    Ok(response) => {
                        info!("Unexpected response fetching configuration: {:?}", response);
                        tx.replace(None);
                    }
                    Err(err) => {
                        warn!("Error fetching configuration: {}", err);
                        tx.replace(None);
                    }
                }
            } else {
                tx.replace(None);
                let _ = params.ipc_endpoint.changed().await;
            }
        }
    });

    rx
}

pub fn watch_ipc_endpoint(
    join_set: &mut JoinSet<()>,
    gateway_configuration: &Receiver<Option<GatewayConfiguration>>,
    tx: Sender<Option<SocketAddr>>,
) {
    let gateway_configuration = gateway_configuration.clone();

    join_set.spawn(async move {
        loop {
            let current_configuration = gateway_configuration.current();
            let primary_endpoint = current_configuration
                .as_ref()
                .as_ref()
                .and_then(|c| c.ipc().clone())
                .and_then(|c| c.endpoint().clone());

            tx.replace(primary_endpoint);

            continue_on!(gateway_configuration.changed());
        }
    });
}
