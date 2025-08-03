mod cli;
mod controllers;
mod proxy;
mod util;

use crate::cli::Cli;
use crate::controllers::config::fs::{WatchConfigurationFileParams, watch_configuration_file};
use crate::controllers::config::ipc::{
    FetchConfigurationParams, fetch_configuration, watch_ipc_endpoint,
};
use crate::controllers::config::selector::{SelectorParams, select_configuration};
use crate::controllers::ipc_events::{PollGatewayEventsParams, poll_gateway_events};
use crate::controllers::router::synthesize_http_router;
use crate::proxy::Proxy;
use clap::Parser;
use kubera_core::crypto::init_crypto;
use kubera_core::instrumentation::init_instrumentation;
use kubera_core::sync::signal::signal;
use pingora::prelude::*;
use pingora::server::Server;
use pingora::services::listening::Service;
use proxy::filters::client_addrs::client_addr_filter;
use proxy::router::topology::TopologyLocation;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    init_crypto();
    init_instrumentation();

    let args = Cli::parse();

    let current_location = {
        let zone = args.zone_name().filter(|z| !z.is_empty());
        let node = args.node_name().filter(|n| !n.is_empty());

        TopologyLocation::builder().zone(zone).node(node).build()
    };

    let (ipc_endpoint_tx, ipc_endpoint_rx) = signal();

    let mut join_set = JoinSet::new();

    let gateway_events_tx = {
        let params = PollGatewayEventsParams::builder()
            .ipc_endpoint_rx(ipc_endpoint_rx.clone())
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name())
            .build();

        poll_gateway_events(&mut join_set, params)
    };

    let ipc_configuration_source_rx = {
        let params = FetchConfigurationParams::builder()
            .ipc_endpoint_rx(ipc_endpoint_rx)
            .gateway_events_rx(gateway_events_tx.subscribe())
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name())
            .build();

        fetch_configuration(&mut join_set, params)
    };

    let fs_configuration_source_rx = {
        let params = WatchConfigurationFileParams::builder()
            .file_path(args.config_file_path())
            .build();

        watch_configuration_file(&mut join_set, params)
    };

    let gateway_configuration_rx = {
        let params = SelectorParams::builder()
            .ipc_configuration_source_rx(ipc_configuration_source_rx)
            .fs_configuration_source_rx(fs_configuration_source_rx)
            .build();

        select_configuration(&mut join_set, params)
    };

    watch_ipc_endpoint(&mut join_set, &gateway_configuration_rx, ipc_endpoint_tx);

    let router_rx =
        synthesize_http_router(&mut join_set, &gateway_configuration_rx, current_location);
    let client_addr_filter_rx = client_addr_filter(&mut join_set, &gateway_configuration_rx);

    join_set.spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let proxy = Proxy::builder()
            .client_addr_filter_rx(client_addr_filter_rx)
            .router_rx(router_rx)
            .build();
        let mut service = http_proxy_service(&server.configuration, proxy);
        service.add_tcp("0.0.0.0:8080");

        server.add_service(service);

        let mut prometheus_service_http = Service::prometheus_http_service();
        prometheus_service_http.add_tcp("0.0.0.0:1234");

        server.add_service(prometheus_service_http);

        server.run_forever();
    });

    join_set.join_all().await;
}
