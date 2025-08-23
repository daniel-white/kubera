mod cli;
mod controllers;
mod http;
mod instrumentation;
mod proxy;
mod util;

use crate::cli::Cli;
use crate::controllers::config::fs::{watch_configuration_file, WatchConfigurationFileParams};
use crate::controllers::config::ipc::{
    fetch_configuration, watch_ipc_endpoint, FetchConfigurationParams,
};
use crate::controllers::config::selector::{select_configuration, SelectorParams};
use crate::controllers::ipc_events::{poll_gateway_events, PollGatewayEventsParams};
use crate::controllers::router::synthesize_http_router;
use crate::controllers::static_response_bodies_cache::static_response_bodies_cache;
use crate::http::filters::access_control::access_control_filters_handlers;
use crate::http::filters::client_addrs::client_addr_filter_handler;
use crate::proxy::filters::static_responses::static_responses;
use crate::proxy::responses::error_responses::error_responses;
use crate::proxy::Proxy;
use clap::Parser;
use pingora::prelude::http_proxy_service;
use pingora::server::Server;

use proxy::router::topology::TopologyLocation;
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;
use vg_core::crypto::init_crypto;
use vg_core::instrumentation::init_instrumentation;
use vg_core::sync::signal::signal;
use vg_core::task::Builder as TaskBuilder;

#[tokio::main]
async fn main() {
    let task_builder = TaskBuilder::default();

    init_crypto();
    init_instrumentation(&task_builder, "vg-gateway");

    let client = reqwest::ClientBuilder::new()
        .build()
        .expect("Failed to create HTTP client");

    let client = Arc::new(
        ClientBuilder::new(client)
            .with(TracingMiddleware::default())
            .build(),
    );

    let args = Cli::parse();

    let current_location = {
        let zone = args.zone_name().filter(|z| !z.is_empty());
        let node = args.node_name().filter(|n| !n.is_empty());

        TopologyLocation::builder().zone(zone).node(node).build()
    };

    let (ipc_endpoint_tx, ipc_endpoint_rx) = signal("ipc_endpoint");

    let gateway_events_tx = {
        let params = PollGatewayEventsParams::builder()
            .client(client.clone())
            .ipc_endpoint_rx(ipc_endpoint_rx.clone())
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name())
            .build();

        poll_gateway_events(&task_builder, params)
    };

    let ipc_configuration_source_rx = {
        let params = FetchConfigurationParams::builder()
            .client(client.clone())
            .ipc_endpoint_rx(ipc_endpoint_rx.clone())
            .gateway_events_rx(gateway_events_tx.subscribe())
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name())
            .build();

        fetch_configuration(&task_builder, params)
    };

    let fs_configuration_source_rx = {
        let params = WatchConfigurationFileParams::builder()
            .file_path(args.config_file_path())
            .build();

        watch_configuration_file(&task_builder, params)
    };

    let gateway_configuration_rx = {
        let params = SelectorParams::builder()
            .ipc_configuration_source_rx(ipc_configuration_source_rx)
            .fs_configuration_source_rx(fs_configuration_source_rx)
            .build();

        select_configuration(&task_builder, params)
    };

    watch_ipc_endpoint(&task_builder, &gateway_configuration_rx, ipc_endpoint_tx);

    let router_rx =
        synthesize_http_router(&task_builder, &gateway_configuration_rx, current_location);
    let client_addr_filter_handler_rx =
        client_addr_filter_handler(&task_builder, &gateway_configuration_rx);
    let access_control_filters_handlers_rx =
        access_control_filters_handlers(&task_builder, &gateway_configuration_rx);
    let error_responses_rx = error_responses(&task_builder, &gateway_configuration_rx);
    let static_responses_rx = static_responses(&task_builder, &gateway_configuration_rx);
    let static_response_bodies_cache = static_response_bodies_cache(
        &task_builder,
        client.clone(),
        &static_responses_rx,
        &ipc_endpoint_rx,
        args.pod_name(),
        args.pod_namespace(),
        args.gateway_name(),
    );

    task_builder.new_task("server").spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let proxy = Proxy::builder()
            .client_addr_filter_handler_rx(client_addr_filter_handler_rx)
            .access_control_filters_handlers_rx(access_control_filters_handlers_rx)
            .error_responses_rx(error_responses_rx)
            .router_rx(router_rx)
            .static_responses_rx(static_responses_rx)
            .static_response_bodies_cache(static_response_bodies_cache)
            .build();
        let mut service = http_proxy_service(&server.configuration, proxy);
        service.add_tcp("0.0.0.0:8080");

        server.add_service(service);

        server.run_forever();
    });

    task_builder.join_all().await;
}
