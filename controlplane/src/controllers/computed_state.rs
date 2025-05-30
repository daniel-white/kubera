use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::controllers::source_controller::{ResourceState, Resources};
use crate::sync::state::Receiver;
use derive_builder::Builder;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use log::debug;
use thiserror::Error;
use tokio::signal;
use tokio::task::JoinSet;

#[derive(Builder)]
pub struct Sources {
    gateway_classes: Receiver<Resources<GatewayClass>>,
    gateway_class_parameters: Receiver<Resources<GatewayClassParameters>>,
    gateways: Receiver<Resources<Gateway>>,
    gateway_parameters: Receiver<Resources<GatewayParameters>>,
    config_maps: Receiver<Resources<ConfigMap>>,
    deployments: Receiver<Resources<Deployment>>,
    services: Receiver<Resources<Service>>,
}

impl Sources {
    pub fn new_builder() -> SourcesBuilder {
        SourcesBuilder::default()
    }
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("error querying computed state")]
    QueryError,
}

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    mut sources: Sources,
) -> Result<Receiver<Option<()>>, ControllerError> {
    let (state_tx, state_rx) = crate::sync::state::channel::<Option<()>>(None);

    join_set.spawn(async move {
        loop {
            tokio::select! {
                _ = sources.gateway_classes.changed() => {
                    debug!("GatewayClasses changed");
                    continue;
                },
                _ = sources.gateway_class_parameters.changed() => {
                    debug!("GatewayClassParameters changed");
                    continue;
                },
                _ = sources.gateways.changed() => {
                    debug!("Gateways changed");
                    continue;
                },
                _ = sources.gateway_parameters.changed() => {
                    continue;
                },
                _ = sources.config_maps.changed() => {
                    continue;
                },
                _ = sources.deployments.changed() => {
                    continue;
                },
                _ = sources.services.changed() => {
                    debug!("Services changed");
                    continue;
                },
                _ = signal::ctrl_c() => {
                    // Handle graceful shutdown
                    break;
                },
            }
        }
    });

    Ok(state_rx)
}
