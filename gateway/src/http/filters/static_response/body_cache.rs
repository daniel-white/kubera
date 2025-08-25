use bytes::Bytes;
use dashmap::DashMap;
use http::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_tracing::{OtelName, OtelPathNames};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, warn};
use typed_builder::TypedBuilder;
use url::Url;
use vg_core::http::filters::static_response::{
    HttpStaticResponseBodyKey, HttpStaticResponseFilterKey,
};

#[derive(Debug, Clone)]
pub struct HttpStaticResponseFilterBodyCache {
    cache: Arc<DashMap<(HttpStaticResponseFilterKey, HttpStaticResponseBodyKey), Arc<Bytes>>>,
}

impl HttpStaticResponseFilterBodyCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::default(),
        }
    }

    pub fn get(
        &self,
        filter_key: &HttpStaticResponseFilterKey,
        body_key: &HttpStaticResponseBodyKey,
    ) -> Option<Arc<Bytes>> {
        self.cache
            .get(&(filter_key.clone(), body_key.clone()))
            .map(|item| item.clone())
    }

    pub fn insert(
        &self,
        filter_key: &HttpStaticResponseFilterKey,
        body_key: &HttpStaticResponseBodyKey,
        body: Bytes,
    ) -> Arc<Bytes> {
        let body = Arc::new(body);
        self.cache
            .insert((filter_key.clone(), body_key.clone()), body.clone());
        body
    }

    pub fn clear(&self) {
        self.cache.clear();
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }
}

pub struct HttpStaticResponseFilterBodyCacheClientState {
    client: Arc<ClientWithMiddleware>,
    ipc_endpoint: SocketAddr,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

#[derive(Debug, Clone, TypedBuilder)]
pub struct HttpStaticResponseFilterBodyCacheClient {
    filter_key: HttpStaticResponseFilterKey,
    cache: HttpStaticResponseFilterBodyCache,
    client: Arc<ClientWithMiddleware>,
    ipc_endpoint: SocketAddr,
    pod_name: String,
    gateway_namespace: String,
    gateway_name: String,
}

impl PartialEq for HttpStaticResponseFilterBodyCacheClient {
    fn eq(&self, other: &Self) -> bool {
        // The only thing that can change is the IPC endpoint
        self.filter_key == other.filter_key &&
        self.ipc_endpoint == other.ipc_endpoint
    }
}

impl Eq for HttpStaticResponseFilterBodyCacheClient {}

impl HttpStaticResponseFilterBodyCacheClient {
    pub async fn get(&self, body_key: &HttpStaticResponseBodyKey) -> Option<Arc<Bytes>> {
        match self.cache.get(&self.filter_key, body_key) {
            Some(cached) => Some(cached),
            None => {
                let url = {
                    let mut url = Url::parse(&format!("http://{}", self.ipc_endpoint))
                        .expect("Failed to parse URL");
                    url.set_path(&format!(
                        "/ipc/namespaces/{}/gateways/{}/static_responses/{}",
                        self.gateway_namespace,
                        self.gateway_name,
                        body_key.into()
                    ));
                    url.set_query(Some(&format!("pod_name={}", self.pod_name)));
                    url
                };

                debug!("Fetching static response from URL: {}", url);

                let response = self.client.get(url)
                    .with_extension(OtelName("get_http_static_response_body".into()))
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
                            Ok(body) => {
                                let body = self.cache.insert(&self.filter_key, body_key, body);
                                Some(body)
                            }
                            Err(err) => {
                                warn!("Error reading response body: {}", err);
                                None
                            }
                        }
                    }
                    Ok(response) => {
                        warn!("Unexpected response fetching configuration: {:?}", response);
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
