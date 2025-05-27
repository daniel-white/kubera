mod gateway_class;
mod gateway_class_parameters;
mod gateways;
mod state;

use anyhow::Result;
use derive_builder::Builder;
use derive_getters::Getters;
use kube::Client;
use tokio::{join, spawn};

pub async fn run() -> Result<()> {
    let client = Client::try_default().await?;

    let (gateway_class_controller, gateway_class_state_rx) =
        gateway_class::controller(&client).await?;
    let (gateway_class_parameters_controller, mut gateway_class_parameters_state_rx) =
        gateway_class_parameters::controller(&client, &gateway_class_state_rx).await?;
    let (gateways_controller, _gateways_state_rx) =
        gateways::controller(&client, &gateway_class_state_rx).await?;

    let x = spawn(async move {
        while let Some(s) = gateway_class_parameters_state_rx.changed().await {
            dbg!(
                "current state:",
                gateway_class_parameters_state_rx.current()
            );
        }
    });

    let _ = join!(
        gateway_class_controller,
        gateway_class_parameters_controller,
        gateways_controller,
        x
    );

    Ok(())
}

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash)]
#[builder(setter(into))]
pub struct Ref {
    name: String,

    namespace: Option<String>,
}

impl Ref {
    pub fn new_builder() -> RefBuilder {
        RefBuilder::default()
    }
}
