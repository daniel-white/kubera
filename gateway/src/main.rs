use crate::http::gateway::{Gateway, GatewayBuilder};
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
    let router = config::router_controller::spawn_controller(config)
        .await
        .expect("Failed to spawn router controller");

    join!(spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let gateway = GatewayBuilder::default()
            .router(router)
            .build()
            .expect("Failed to build gateway");
        let mut service = http_proxy_service(&server.configuration, gateway);
        service.add_tcp("0.0.0.0:8080");

        server.add_service(service);
        server.run_forever();
    }));
}
