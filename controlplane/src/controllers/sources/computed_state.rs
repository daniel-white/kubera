use crate::api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use crate::controllers::sources::controller::{ResourceState, Resources};
use crate::sync::state::Receiver;
use derive_builder::Builder;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use thiserror::Error;
use tokio::signal;
use tokio::task::JoinSet;

#[derive(Builder)]
pub struct StateSources {
    gateway_classes: Receiver<Resources<GatewayClass>>,
    gateway_class_parameters: Receiver<Resources<GatewayClassParameters>>,
    gateways: Receiver<Resources<Gateway>>,
    gateway_parameters: Receiver<Resources<GatewayParameters>>,
    config_maps: Receiver<Resources<ConfigMap>>,
    deployments: Receiver<Resources<Deployment>>,
    services: Receiver<Resources<Service>>,
}

impl StateSources {
    pub fn new_builder() -> StateSourcesBuilder {
        StateSourcesBuilder::default()
    }
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("error querying computed state")]
    QueryError,
}

pub async fn spawn_controller(
    join_set: &mut JoinSet<()>,
    mut receivers: StateSources,
) -> Result<Receiver<Option<()>>, ControllerError> {
    let (state_tx, state_rx) = crate::sync::state::channel::<Option<()>>(None);

    join_set.spawn(async move {
        loop {
            tokio::select! {
                _ = receivers.gateway_classes.changed() => {
                    continue;
                },
                _ = receivers.gateway_class_parameters.changed() => {
                    continue;
                },
                _ = receivers.gateways.changed() => {
                    continue;
                },
                _ = receivers.gateway_parameters.changed() => {
                    continue;
                },
                _ = receivers.config_maps.changed() => {
                    continue;
                },
                _ = receivers.deployments.changed() => {
                    continue;
                },
                _ = receivers.services.changed() => {
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
