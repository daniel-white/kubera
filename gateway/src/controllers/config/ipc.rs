use http::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_tracing::{OtelName, OtelPathNames};
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast::Receiver as BroadcastReceiver;
use tracing::{debug, info, warn};
use typed_builder::TypedBuilder;
use url::Url;
use vg_core::gateways::Gateway;
use vg_core::ipc::GatewayEvent;
use vg_core::sync::signal::{signal, Receiver, Sender};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

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

pub fn ipc_source(
    task_builder: &TaskBuilder,
    params: FetchConfigurationParams,
) -> Receiver<(Instant, Gateway)> {
    let ipc_endpoint_rx = params.ipc_endpoint_rx.clone();
    let (tx, rx) = signal("ipc_source");

    task_builder
        .new_task(stringify!(fetch_configuration))
        .spawn(async move {
            let mut gateway_events = params.gateway_events_rx;
            loop {
                if let ReadyState::Ready(ipc_endpoint_addr) = await_ready!(ipc_endpoint_rx)
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

                    let response = params
                        .client
                        .get(url)
                        .with_extension(OtelName("fetch_configuration".into()))
                        .with_extension(
                            OtelPathNames::known_paths([
                                "/ipc/namespaces/{namespace}/gateways/{gateway_name}/configuration",
                            ])
                            .expect("Failed to set known paths"),
                        )
                        .send()
                        .await;

                    match response {
                        Ok(response) if response.status() == StatusCode::OK => {
                            match response.bytes().await {
                                Ok(bytes) => {
                                    let gateway: Gateway = serde_yaml::from_slice(bytes.as_ref())
                                        .expect("Failed to deserialize Gateway");
                                    tx.set((serial, gateway)).await;
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

pub fn ipc_addr(
    task_builder: &TaskBuilder,
    gateway_rx: &Receiver<Gateway>,
    tx: Sender<SocketAddr>,
) {
    let gateway_rx = gateway_rx.clone();

    task_builder
        .new_task(stringify!(watch_ipc_endpoint))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway) = await_ready!(gateway_rx) {
                    let addr = gateway.ipc().addr();
                    tx.replace(addr).await;
                }

                continue_on!(gateway_rx.changed());
            }
        });
}
