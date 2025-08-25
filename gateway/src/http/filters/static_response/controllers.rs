use std::collections::HashMap;
use vg_core::http::filters::access_control::{HttpAccessControlFilter, HttpAccessControlFilterKey};
use vg_core::http::filters::static_response::{HttpStaticResponseBody, HttpStaticResponseFilter, HttpStaticResponseFilterKey};
use vg_core::http::listeners::{HttpFilterDefinition, HttpListener};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};
use crate::http::filters::static_response::body_cache::{HttpStaticResponseFilterBodyCache, HttpStaticResponseFilterBodyCacheClient};

fn http_static_response_filters(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>
) -> Receiver<HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>> {
    let (tx, rx) = signal(stringify!(http_static_response_filters));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(http_static_response_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let filters = match http_listener.as_ref() {
                        Some(http_listener) => http_listener
                            .filter_definitions()
                            .iter()
                            .filter_map(|f| match f {
                                HttpFilterDefinition::StaticResponse(filter) => {
                                    Some((filter.key().clone(), filter.clone()))
                                }
                                _ => None,
                            })
                            .collect(),
                        None => HashMap::new(),
                    };

                    tx.set(filters).await;
                }
                continue_on!(http_listener_rx.changed());
            }
        });

    rx
}

fn http_static_response_filter_body_cache(
    task_builder: &TaskBuilder,
    filters_rx: &Receiver<HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>>
) -> HttpStaticResponseFilterBodyCache {
    let cache = HttpStaticResponseFilterBodyCache::new();
    
    let cache_to_clear = cache.clone();
    task_builder
        .new_task(stringify!(http_static_response_filter_body_cache))
        .spawn({
            let filters_rx = filters_rx.clone();
            async move {
                loop {
                    if let ReadyState::Ready(_) = await_ready!(filters_rx) {
                        cache_to_clear.clear();
                    }
                    continue_on!(filters_rx.changed());
                }
            }
        });
    cache
}


pub fn http_static_response_filter_handlers(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>> {
    let (tx, rx) = signal(stringify!(http_static_response_filter_handlers));
    let filters_rx = http_static_response_filters(task_builder, http_listener_rx);
    let body_cache = http_static_response_filter_body_cache(task_builder, &filters_rx);

    task_builder
        .new_task(stringify!(http_static_response_filter_handlers))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(filters_rx) {
                    let handlers = filters
                        .iter()
                        .map(|(key, filter)| {
                            // TODO impl
                            (key.clone(), filter.clone())
                        })
                        .collect();
                    tx.set(handlers).await;
                }
                continue_on!(filters_rx.changed());
            }
        });
    rx
}
