use crate::http::gateway::Gateway;
use kubera_core::config::logging::init_logging;
use pingora::prelude::*;
use pingora::server::Server;
use tokio::join;
use tokio::task::spawn_blocking;

mod config;
mod http;
mod util;

#[tokio::main]
async fn main() {
    init_logging();

    let config = config::config_watcher_controller::spawn_controller("gateway.yaml")
        .expect("Failed to spawn controller");
    let matchers = config::matchers_controller::spawn_controller(config)
        .await
        .expect("Failed to spawn matchers controller");

    join!(spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let mut service = http_proxy_service(&server.configuration, Gateway::new(matchers));
        service.add_tcp("0.0.0.0:8080");

        server.add_service(service);
        server.run_forever();
    }));
}
