use crate::controllers::sources::deployments::DeploymentsState;
use crate::controllers::sources::gateway_class::GatewayClassState;
use crate::controllers::sources::gateway_class_parameters::GatewayClassParametersState;
use crate::controllers::sources::gateway_parameters::GatewayParametersState;
use crate::controllers::sources::gateways::GatewaysState;
use crate::controllers::sources::services::ServicesState;
use crate::sync::state::Receiver;
use derive_builder::Builder;
use thiserror::Error;
use tokio::signal;
use tokio::task::{JoinHandle, JoinSet};

#[derive(Builder)]
pub struct StateSources {
    gateway_class: Receiver<Option<GatewayClassState>>,
    gateway_class_parameters: Receiver<Option<GatewayClassParametersState>>,
    gateways: Receiver<GatewaysState>,
    gateway_parameters: Receiver<GatewayParametersState>,
    deployments: Receiver<DeploymentsState>,
    services: Receiver<ServicesState>,
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
                _ = receivers.gateway_class.changed() => {
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
