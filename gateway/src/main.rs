mod config;
mod http;
pub mod net;
mod util;

use crate::http::proxy::ProxyBuilder;
use crate::net::resolver::SocketAddrResolver;
use kubera_core::config::logging::init_logging;
use pingora::prelude::*;
use pingora::server::Server;
use tokio::join;
use tokio::task::spawn_blocking;

#[tokio::main]
async fn main() {
    init_logging();

    let config = config::config_watcher_controller::spawn_controller("gateway.yaml")
        .expect("Failed to spawn controller");
    let router = config::router_controller::spawn_controller(config)
        .await
        .expect("Failed to spawn router controller");
    let socket_addr_resolver = SocketAddrResolver::new();

    join!(spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let proxy = ProxyBuilder::default()
            .router(router)
            .addr_resolver(socket_addr_resolver)
            .build()
            .expect("Failed to build proxy");
        let mut service = http_proxy_service(&server.configuration, proxy);
        service.add_tcp("0.0.0.0:8080");

        server.add_service(service);
        server.run_forever();
    }));
}
