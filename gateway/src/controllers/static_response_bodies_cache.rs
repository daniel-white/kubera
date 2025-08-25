use bytes::Bytes;
use dashmap::DashMap;
use dashmap::Entry::*;
use http::{HeaderValue, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_tracing::{OtelName, OtelPathNames};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use url::Url;
use vg_core::http::filters::static_response::{
    HttpStaticResponseBodyKey, HttpStaticResponseFilter, HttpStaticResponseFilterKey,
};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

#[derive(Debug)]
struct StaticResponseBodiesCacheState {
    cache: DashMap<HttpStaticResponseBodyKey, (HeaderValue, Arc<Bytes>)>,
    responses: HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>,
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
    pub async fn get(
        &self,
        key: &HttpStaticResponseFilterKey,
    ) -> Option<(HeaderValue, Arc<Bytes>)> {
        let state = self.state.read().await;
        let state = state.as_ref()?;
        let response = state.responses.get(key)?;

        let body = match response.body() {
            None => return None,
            Some(body) => body,
        };

        match state.cache.entry(body.key().clone()) {
            Occupied(entry) => Some(entry.get().clone()),
            Vacant(entry) => {}
        }
    }
}

pub fn static_response_bodies_cache(
    task_builder: &TaskBuilder,
    client: Arc<ClientWithMiddleware>,
    static_responses_rx: &Receiver<
        Arc<HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>>,
    >,
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
