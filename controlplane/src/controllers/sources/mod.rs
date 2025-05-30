mod computed_state;
mod deployments;
mod gateway_class;
mod gateway_class_parameters;
mod gateway_parameters;
mod gateways;
mod services;

use crate::sync::state::Receiver;
use anyhow::Result;
use kube::Client;
use tokio::task::JoinSet;

pub async fn spawn_sources(
    join_set: &mut JoinSet<()>,
    client: &Client,
) -> Result<Receiver<Option<()>>> {
    let gateway_class_state = gateway_class::spawn_controller(join_set, client).await?;
    let gateway_class_parameters_state =
        gateway_class_parameters::spawn_controller(join_set, client, &gateway_class_state).await?;
    let gateways_state = gateways::spawn_controller(join_set, client, &gateway_class_state).await?;
    let gateway_parameters_state = gateway_parameters::spawn_controller(join_set, client).await?;
    let deployments_state = deployments::spawn_controller(join_set, client).await?;
    let services_state = services::spawn_controller(join_set, client).await?;

    let state_sources = computed_state::StateSources::new_builder()
        .gateway_class(gateway_class_state)
        .gateway_class_parameters(gateway_class_parameters_state)
        .gateways(gateways_state)
        .gateway_parameters(gateway_parameters_state)
        .deployments(deployments_state)
        .services(services_state)
        .build()?;
    let computed_state = computed_state::spawn_controller(join_set, state_sources).await?;

    Ok(computed_state)
}
