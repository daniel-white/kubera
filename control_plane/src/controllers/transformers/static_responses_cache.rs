use crate::kubernetes::objects::{ObjectUniqueId, Objects};
use base64ct::{Base64UrlUnpadded, Encoding};
use bytes::Bytes;
use dashmap::DashMap;
use dashmap::Entry::{Occupied, Vacant};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::{StaticResponseFilter, StaticResponseFilterBodyFormat};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{ReadyState, await_ready, continue_on};

#[derive(Debug, Default)]
struct StaticResponseCacheState {
    cache: DashMap<ObjectUniqueId, (String, Bytes)>,
    filters_rx: Option<Receiver<Objects<StaticResponseFilter>>>,
}

#[derive(Debug, Default, TypedBuilder, Clone)]
pub struct StaticResponsesCache {
    data: Arc<RwLock<StaticResponseCacheState>>,
}

impl StaticResponsesCache {
    pub async fn get(&self, key: ObjectUniqueId) -> Option<(String, Bytes)> {
        let data = self.data.read().await;
        match data.cache.entry(key.clone()) {
            Occupied(entry) => Some(entry.get().clone()),
            Vacant(entry) => {
                if let Some(filters_rx) = &data.filters_rx
                    && let Some(filters) = filters_rx.get().await.as_ref()
                    && let Some(filter) = filters.get_by_unique_id(&key)
                    && let Some(body) = &filter.spec.body
                {
                    let bytes = match &body.format {
                        StaticResponseFilterBodyFormat::Text => {
                            if let Some(text) = &body.text {
                                Bytes::from(text.clone().into_bytes())
                            } else {
                                warn!("Text format static response body is missing text content");
                                return None;
                            }
                        }
                        StaticResponseFilterBodyFormat::Binary => {
                            if let Some(binary) = &body.binary {
                                match Base64UrlUnpadded::decode_vec(binary) {
                                    Ok(bytes) => Bytes::from(bytes),
                                    Err(err) => {
                                        warn!("Failed to decode base64 binary content: {}", err);
                                        return None;
                                    }
                                }
                            } else {
                                warn!(
                                    "Binary format static response body is missing binary content"
                                );
                                return None;
                            }
                        }
                    };

                    let value = (body.content_type.clone(), bytes);
                    entry.insert(value.clone());
                    return Some(value);
                }
                None
            }
        }
    }

    pub async fn reset(&mut self, filters_rx: Receiver<Objects<StaticResponseFilter>>) {
        let mut data = self.data.write().await;
        data.cache.clear();
        data.filters_rx = Some(filters_rx);
    }
}

pub fn bind_static_responses_cache(
    task_builder: &TaskBuilder,
    static_response_filters_rx: &Receiver<Objects<StaticResponseFilter>>,
    static_responses_cache: StaticResponsesCache,
) {
    let static_response_filters_rx = static_response_filters_rx.clone();
    let mut static_responses_cache = static_responses_cache.clone();

    task_builder
        .new_task(stringify!(static_responses_cache))
        .spawn(async move {
            loop {
                let filters_rx = static_response_filters_rx.clone();
                if let ReadyState::Ready(_) = await_ready!(static_response_filters_rx) {
                    static_responses_cache.reset(filters_rx).await;
                }
                continue_on!(static_response_filters_rx.changed())
            }
        });
}
