mod cli;
mod config;
mod controllers;
mod services;
mod util;

use crate::cli::Cli;
use crate::config::config_watcher_controller::ConfigurationReaderParameters;
use crate::config::topology::TopologyLocationBuilder;
use crate::controllers::events::{watch_gateway_events, WatchGatewayEventsParams};
use crate::services::proxy::ProxyBuilder;
use clap::Parser;
use futures::task::SpawnExt;
use kubera_core::config::logging::init_logging;
use kubera_core::continue_on;
use once_cell::sync::Lazy;
use pingora::prelude::*;
use pingora::server::Server;
use pingora::services::listening::Service;
use prometheus::{register_int_gauge, IntGauge};
use std::path::PathBuf;
use tokio::join;
use tokio::task::{spawn_blocking, JoinSet};
use tracing::warn;

static MY_COUNTER: Lazy<IntGauge> =
    Lazy::new(|| register_int_gauge!("my_counter", "my counter").unwrap());

#[tokio::main]
async fn main() {
    init_logging();
    let cli = Cli::parse();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let current_location = {
        let zone_name = cli.zone_name().filter(|z| !z.is_empty());
        let node_name = cli.node_name().filter(|n| !n.is_empty());

        let mut current_location = TopologyLocationBuilder::default();
        current_location.in_zone(&zone_name).on_node(&node_name);
        current_location.build()
    };

    let configuration_reader_params = {
        ConfigurationReaderParameters::<PathBuf>::new_builder()
            .config_path(cli.config_file_path())
            .gateway_name(cli.gateway_name())
            .gateway_namespace(cli.pod_namespace())
            .build()
            .expect("Failed to build ConfigurationReaderParameters")
    };

    warn!("Current location: {:?}", current_location);
    warn!(
        "Configuration reader parameters: {:?}",
        configuration_reader_params
    );

    let (tx, rx) = kubera_core::sync::signal::channel(None);

    let config = config::config_watcher_controller::spawn_controller(configuration_reader_params)
        .expect("Failed to spawn controller");
    let router = config::router_controller::spawn_controller(config.clone(), current_location)
        .await
        .expect("Failed to spawn router controller");

    let mut join_set = JoinSet::new();

    let p = WatchGatewayEventsParams::new_builder()
        .primary_socket_addr(rx)
        .pod_name(cli.pod_name())
        .gateway_namespace(cli.pod_namespace())
        .gateway_name(cli.gateway_name())
        .build()
        .expect("Failed to build WatchGatewayEventsParams");

    watch_gateway_events(&mut join_set, p);

    join_set.spawn(async move {
        loop {
            let cc = config
                .current()
                .as_ref()
                .clone()
                .and_then(|c| c.controlplane().clone())
                .and_then(|c| c.primary_endpoint().clone());
            tx.replace(cc);

            continue_on!(config.changed());
        }
    });

    join_set.join_all().await;

    join!(spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let proxy = ProxyBuilder::default()
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
    }));
}
