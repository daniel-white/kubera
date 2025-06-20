mod cli;
mod config;
mod services;
mod util;

use crate::cli::Cli;
use crate::config::topology::TopologyLocationBuilder;
use crate::services::control::spawn_control_server;
use crate::services::proxy::ProxyBuilder;
use clap::Parser;
use kubera_core::config::logging::init_logging;
use once_cell::sync::Lazy;
use pingora::prelude::*;
use pingora::server::Server;
use pingora::services::listening::Service;
use prometheus::{register_int_gauge, IntGauge};
use tokio::join;
use tokio::task::spawn_blocking;

static MY_COUNTER: Lazy<IntGauge> =
    Lazy::new(|| register_int_gauge!("my_counter", "my counter").unwrap());

#[tokio::main]
async fn main() {
    init_logging();
    let cli = Cli::parse();

    let zone = cli.kubernetes_zone_name().clone().filter(|z| !z.is_empty());
    let node = cli.kubernetes_node_name().clone().filter(|n| !n.is_empty());

    let mut current_location = TopologyLocationBuilder::default();
    current_location.in_zone(&zone).on_node(&node);
    let current_location = current_location.build();

    let config = config::config_watcher_controller::spawn_controller("gateway.yaml")
        .expect("Failed to spawn controller");
    let router = config::router_controller::spawn_controller(config, current_location)
        .await
        .expect("Failed to spawn router controller");

    join!(
        spawn_control_server(cli.control_service_port()),
        spawn_blocking(move || {
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
        })
    );
}
