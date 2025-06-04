//use pingora::gateway::{Proxy, ProxyConfig};
//use tokio::runtime::Runtime;

use kubera_core::select_continue;
use std::time::Duration;
use tokio::time::sleep;
use tokio::{join, spawn};

mod config;
mod http;
mod util;

#[tokio::main]
async fn main() {
    let config = config::config_watcher_controller::spawn_controller("gateway.yaml")
        .expect("Failed to spawn controller");
    let matchers = config::matchers_controller::spawn_controller(config)
        .await
        .expect("Failed to spawn matchers controller");

    let x = spawn(async move {
        let mut matchers = matchers.clone();
        sleep(Duration::from_secs(1)).await; // Wait for initial configuration
        loop {
            // Here you would typically use the matchers to configure your HTTP server
            // For example, you could print them or use them to set up routes
            println!("Current matchers: {:?}", matchers.current());

            select_continue!(matchers.changed());
        }
    });

    join!(x);
}
