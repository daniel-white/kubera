use super::extractors::ClientAddrExtractorType::*;
use super::extractors::{
    NoopClientAddrExtractor, TrustedHeaderClientAddrExtractor, TrustedProxiesClientAddrExtractor,
};
use super::ClientAddrFilterHandler;
use std::collections::HashMap;
use vg_core::http::filters::client_addrs::{
    HttpClientAddrFilterKey, HttpClientAddrsFilter, HttpClientAddrsSource, HttpProxyHeaders,
};
use vg_core::http::listeners::{HttpFilterDefinition, HttpListener};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

fn http_client_addr_filters(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<HashMap<HttpClientAddrFilterKey, HttpClientAddrsFilter>> {
    let (tx, rx) = signal(stringify!(access_control_filters));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(access_control_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let filters = match http_listener.as_ref() {
                        Some(http_listener) => http_listener
                            .filter_definitions()
                            .iter()
                            .filter_map(|f| match f {
                                HttpFilterDefinition::ClientAddrs(filter) => {
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

pub fn client_addr_filter_handlers(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<HashMap<HttpClientAddrFilterKey, ClientAddrFilterHandler>> {
    let (tx, rx) = signal(stringify!(client_addr_filter_handlers));
    let filters_rx = http_client_addr_filters(task_builder, http_listener_rx);

    task_builder
        .new_task(stringify!(client_addr_filter_handlers))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(filters_rx) {
                    let handlers = filters
                        .iter()
                        .map(|(key, filter)| {
                            let extractor =
                                match (filter.source(), filter.header(), filter.proxies()) {
                                    (HttpClientAddrsSource::Header, Some(header), _) => {
                                        let extractor = TrustedHeaderClientAddrExtractor::builder()
                                            .key(key.clone())
                                            .header(header)
                                            .build();
                                        TrustedHeader(extractor)
                                    }
                                    (HttpClientAddrsSource::Proxies, _, Some(proxies)) => {
                                        let mut extractor =
                                            TrustedProxiesClientAddrExtractor::builder(key.clone());

                                        for ip in proxies.trusted_ips() {
                                            extractor.add_trusted_ip(*ip);
                                        }

                                        for range in proxies.trusted_ranges() {
                                            extractor.add_trusted_ip_range(*range);
                                        }

                                        for header in proxies.trusted_headers() {
                                            match header {
                                                HttpProxyHeaders::Forwarded => {
                                                    extractor.trust_forwarded_header();
                                                }
                                                HttpProxyHeaders::XForwardedFor => {
                                                    extractor.trust_x_forwarded_for_header();
                                                }
                                                HttpProxyHeaders::XForwardedBy => {
                                                    extractor.trust_x_forwarded_by_header();
                                                }
                                                HttpProxyHeaders::XForwardedProto => {
                                                    extractor.trust_x_forwarded_proto_header();
                                                }
                                                HttpProxyHeaders::XForwardedHost => {
                                                    extractor.trust_x_forwarded_host_header();
                                                }
                                            }
                                        }

                                        let extractor = extractor.build();

                                        TrustedProxies(extractor)
                                    }
                                    (source, _, _) => {
                                        // TODO log
                                        let extractor = NoopClientAddrExtractor::builder()
                                            .key(key.clone())
                                            .build();

                                        Noop(extractor)
                                    }
                                };
                            let handler = ClientAddrFilterHandler::builder()
                                .extractor(extractor)
                                .build();
                            (key.clone(), handler)
                        })
                        .collect();
                    tx.set(handlers).await;
                }

                continue_on!(filters_rx.changed());
            }
        });

    rx
}
