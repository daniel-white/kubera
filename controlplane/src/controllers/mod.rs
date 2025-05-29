mod computed_state;
mod deployments;
mod gateway_class;
mod gateway_class_parameters;
mod gateway_parameters;
mod gateways;
mod services;

use anyhow::Result;
use derive_builder::Builder;
use getset::Getters;
use kube::Client;
use tokio::join;

pub async fn run() -> Result<()> {
    let client = Client::try_default().await?;

    let (gateway_class_controller, gateway_class_state) =
        gateway_class::controller(&client).await?;
    let (gateway_class_parameters_controller, gateway_class_parameters_state) =
        gateway_class_parameters::controller(&client, &gateway_class_state).await?;
    let (gateways_controller, gateways_state) =
        gateways::controller(&client, &gateway_class_state).await?;
    let (gateway_parameters_controller, gateway_parameters_state) =
        gateway_parameters::controller(&client).await?;
    let (deployments_controller, deployments_state) = deployments::controller(&client).await?;
    let (services_controller, services_state) = services::controller(&client).await?;
    let (computed_state_controller, _computed_state_rx) = computed_state::controller(
        computed_state::ComputedStateSourceStates::new_builder()
            .gateway_class(gateway_class_state)
            .gateway_class_parameters(gateway_class_parameters_state)
            .gateways(gateways_state)
            .gateway_parameters(gateway_parameters_state)
            .deployments(deployments_state)
            .services(services_state)
            .build()?,
    )
    .await?;

    let _ = join!(
        gateway_class_controller,
        gateway_class_parameters_controller,
        gateways_controller,
        gateway_parameters_controller,
        services_controller,
        computed_state_controller
    );

    Ok(())
}

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash)]
#[builder(setter(into))]
pub struct Ref {
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    namespace: Option<String>,
}

impl Ref {
    pub fn new_builder() -> RefBuilder {
        RefBuilder::default()
    }
}
