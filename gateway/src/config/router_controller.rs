use crate::http::router::{Route, RouteMatcher, Router, RouterBuilder};

use kubera_core::config::gateway::types::GatewayConfiguration;
use kubera_core::select_continue;
use kubera_core::sync::signal::{channel, Receiver};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("Failed to spawn controller")]
    SpawnError,
}

pub async fn spawn_controller(
    gateway_configuration: Receiver<Option<GatewayConfiguration>>,
) -> Result<Receiver<Option<Router>>, ControllerError> {
    let mut gateway_configuration = gateway_configuration.clone();
    let (tx, rx) = channel(None);

    tokio::spawn(async move {
        loop {
            if let Some(gateway_config) = gateway_configuration.current() {
                let mut router_builder = Router::new_builder();
                for host in gateway_config.hosts().iter() {
                    for route in host.http_routes().iter() {
                        router_builder.route(|b| {
                            b.with_method(http::Method::GET);
                        });
                    }
                }
                match router_builder.build() {
                    Ok(router) => {
                        tracing::info!("Router configuration updated");
                        tx.replace(Some(router));
                    }
                    Err(e) => {
                        tracing::error!("Failed to build router: {}", e);
                        tx.replace(None);
                    }
                }
            }

            select_continue!(gateway_configuration.changed())
        }
    });

    Ok(rx)
}
