use bytes::Bytes;
use dashmap::DashMap;
use dashmap::Entry::*;
use http::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_tracing::{OtelName, OtelPathNames};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use url::Url;
use vg_core::config::gateway::types::net::StaticResponse;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

#[derive(Debug)]
struct StaticResponseBodiesCacheState {
    cache: DashMap<String, (String, Arc<Bytes>)>,
    responses: HashMap<String, StaticResponse>,
    client: Arc<ClientWithMiddleware>,
    ipc_endpoint: SocketAddr,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

#[derive(Debug, Default, Clone)]
pub struct StaticResponseBodiesCache {
    state: Arc<RwLock<Option<StaticResponseBodiesCacheState>>>,
}

impl StaticResponseBodiesCache {
    pub async fn get(&self, key: &str) -> Option<(String, Arc<Bytes>)> {
        let state = self.state.read().await;
        let state = state.as_ref()?;
        let response = state.responses.get(key)?;

        let body = match response.body() {
            None => return None,
            Some(body) => body,
        };

        match state.cache.entry(body.identifier().clone()) {
            Occupied(entry) => Some(entry.get().clone()),
            Vacant(entry) => {
                let url = {
                    let mut url = Url::parse(&format!("http://{}", state.ipc_endpoint))
                        .expect("Failed to parse URL");
                    url.set_path(&format!(
                        "/ipc/namespaces/{}/gateways/{}/static_responses/{}",
                        state.gateway_namespace,
                        state.gateway_name,
                        body.identifier()
                    ));
                    url.set_query(Some(&format!("pod_name={}", state.pod_name)));
                    url
                };
                debug!("Fetching static response from URL: {}", url);

                let response = state.client.get(url)
                    .with_extension(OtelName("fetch_static_response".into()))
                    .with_extension(
                        OtelPathNames::known_paths([
                            "/ipc/namespaces/{namespace}/gateways/{gateway_name}/static_responses/{static_response_id}",
                        ])
                            .expect("Failed to set known paths"),
                    )
                    .send()
                    .await;

                match response {
                    Ok(response) if response.status() == StatusCode::OK => {
                        match response.bytes().await {
                            Ok(bytes) => {
                                let value = (body.content_type().clone(), Arc::from(bytes));
                                entry.insert(value.clone());
                                Some(value)
                            }
                            Err(err) => {
                                warn!("Error reading response body: {}", err);
                                None
                            }
                        }
                    }
                    Ok(response) => {
                        info!("Unexpected response fetching configuration: {:?}", response);
                        None
                    }
                    Err(err) => {
                        warn!("Error fetching configuration: {}", err);
                        None
                    }
                }
            }
        }
    }
}

pub fn static_response_bodies_cache(
    task_builder: &TaskBuilder,
    client: Arc<ClientWithMiddleware>,
    static_responses_rx: &Receiver<Arc<HashMap<String, StaticResponse>>>,
    ipc_endpoint_rx: &Receiver<SocketAddr>,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
) -> StaticResponseBodiesCache {
    let cache = StaticResponseBodiesCache::default();
    let static_responses_rx = static_responses_rx.clone();
    let ipc_endpoint_rx = ipc_endpoint_rx.clone();

    let cache_for_task = cache.clone();
    task_builder
        .new_task(stringify!(static_response_bodies_cache))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((static_responses, ipc_endpoint)) =
                    await_ready!(static_responses_rx, ipc_endpoint_rx)
                {
                    let mut state = cache_for_task.state.write().await;
                    state.replace(StaticResponseBodiesCacheState {
                        cache: DashMap::new(),
                        responses: static_responses.as_ref().clone(),
                        client: client.clone(),
                        ipc_endpoint: *ipc_endpoint,
                        pod_name: pod_name.clone(),
                        gateway_namespace: gateway_namespace.clone(),
                        gateway_name: gateway_name.clone(),
                    });
                }
                continue_on!(static_responses_rx.changed(), ipc_endpoint_rx.changed());
            }
        });

    cache
}
