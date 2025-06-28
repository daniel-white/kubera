use getset::Getters;
use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::ipc::GatewayEvent;
use kubera_core::sync::signal::{Receiver, Sender};
use reqwest::Client;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::sync::broadcast::Receiver as BroadcastReceiver;
use tokio::task::JoinSet;
use tracing::{error, info};
use url::Url;

#[derive(Debug, Getters)]
pub struct FetchGatewayConfigurationParams {
    primary_socket_addr: Receiver<Option<SocketAddr>>,
    gateway_events: BroadcastReceiver<GatewayEvent>,
    gateway_configuration: Sender<Option<(Instant, GatewayConfiguration)>>,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

impl FetchGatewayConfigurationParams {
    pub fn new_builder() -> FetchGatewayConfigurationParamsBuilder {
        FetchGatewayConfigurationParamsBuilder::default()
    }
}

#[derive(Default)]
pub struct FetchGatewayConfigurationParamsBuilder {
    primary_socket_addr: Option<Receiver<Option<SocketAddr>>>,
    gateway_events: Option<BroadcastReceiver<GatewayEvent>>,
    gateway_configuration: Option<Sender<Option<(Instant, GatewayConfiguration)>>>,
    pod_name: Option<String>,
    gateway_namespace: Option<String>,
    gateway_name: Option<String>,
}

impl FetchGatewayConfigurationParamsBuilder {
    pub fn primary_socket_addr(&mut self, addr: &Receiver<Option<SocketAddr>>) -> &mut Self {
        self.primary_socket_addr = Some(addr.clone());
        self
    }

    pub fn gateway_events(&mut self, events: BroadcastReceiver<GatewayEvent>) -> &mut Self {
        self.gateway_events = Some(events);
        self
    }

    pub fn gateway_configuration(
        &mut self,
        configuration: &Sender<Option<(Instant, GatewayConfiguration)>>,
    ) -> &mut Self {
        self.gateway_configuration = Some(configuration.clone());
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

    pub fn build(self) -> FetchGatewayConfigurationParams {
        FetchGatewayConfigurationParams {
            primary_socket_addr: self
                .primary_socket_addr
                .expect("Primary socket address is required"),
            gateway_events: self
                .gateway_events
                .expect("Gateway events receiver is required"),
            gateway_configuration: self
                .gateway_configuration
                .expect("Gateway configuration sender is required"),
            pod_name: self.pod_name.expect("Pod name is required"),
            gateway_namespace: self
                .gateway_namespace
                .expect("Gateway namespace is required"),
            gateway_name: self.gateway_name.expect("Gateway name is required"),
        }
    }
}

pub fn fetch_gateway_configuration(
    join_set: &mut JoinSet<()>,
    params: FetchGatewayConfigurationParams,
) {
    join_set.spawn(async move {
        let mut gateway_events = params.gateway_events;
        let client = Client::new();
        loop {
            if let Some(socket_addr) = params.primary_socket_addr.current().as_ref()
                && let Ok(event) = gateway_events.recv().await
                && let GatewayEvent::ConfigurationUpdate(_) = event
            {
                let url = {
                    let mut url = Url::parse(&format!("http://{}", socket_addr))
                        .expect("Failed to parse URL");
                    url.set_path(&format!(
                        "/ipc/namespaces/{}/gateways/{}/configuration",
                        params.gateway_namespace, params.gateway_name
                    ));
                    url.set_query(Some(&format!("pod_name={}", params.pod_name)));
                    url
                };

                info!("Fetching configuration from URL: {}", url);

                match client.get(url).send().await {
                    Ok(response) => {
                        info!("Received response: {:?}", response);
                    }
                    Err(err) => {
                        error!("Failed to fetch configuration: {}", err);
                    }
                }
            }
        }
    });
}
