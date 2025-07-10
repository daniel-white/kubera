mod cli;
mod controllers;
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
use clap::Parser;
use kubera_core::crypto::init_crypto;
use kubera_core::instrumentation::init_instrumentation;
use kubera_core::sync::signal::channel;
use pingora::prelude::*;
use pingora::server::Server;
use pingora::services::listening::Service;
use proxy::filters::client_addrs::client_addr_filter;
use proxy::router::topology::TopologyLocation;
use proxy::ProxyBuilder;
use tokio::task::{JoinSet};

#[tokio::main]
async fn main() {
    init_crypto();
    init_instrumentation();

    let args = Cli::parse();

    let current_location = {
        let zone_name = args.zone_name().filter(|z| !z.is_empty());
        let node_name = args.node_name().filter(|n| !n.is_empty());

        let mut current_location = TopologyLocation::new_builder();
        current_location.in_zone(zone_name).on_node(node_name);
        current_location.build()
    };

    let (ipc_endpoint_tx, ipc_endpoint) = channel(None);

    let mut join_set = JoinSet::new();

    let gateway_events = {
        let mut params = PollGatewayEventsParams::new_builder();
        params
            .ipc_endpoint(&ipc_endpoint)
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name());

        poll_gateway_events(&mut join_set, params.build())
    };

    let ipc_configuration_source = {
        let mut params = FetchConfigurationParams::new_builder();
        params
            .ipc_endpoint(&ipc_endpoint)
            .gateway_events(gateway_events.subscribe())
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name());

        fetch_configuration(&mut join_set, params.build())
    };

    let fs_configuration_source = {
        let mut params = WatchConfigurationFileParams::new_builder();
        params.file_path(args.config_file_path());
        watch_configuration_file(&mut join_set, params.build())
    };

    let config = {
        let mut params = SelectorParams::new_builder();
        params
            .ipc_configuration_source(ipc_configuration_source)
            .fs_configuration_source(fs_configuration_source);

        select_configuration(
            &mut join_set,
            params.build().expect("Failed to build SelectorParams"),
        )
    };

    watch_ipc_endpoint(&mut join_set, &config, ipc_endpoint_tx);

    let router = synthesize_http_router(&mut join_set, &config, current_location);
    let client_addr_filter = client_addr_filter(&mut join_set, &config);

    join_set.spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let proxy = ProxyBuilder::default()
            .client_addr_filter(client_addr_filter)
            .router(router)
            .build()
            .expect("Failed to build proxy");
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
