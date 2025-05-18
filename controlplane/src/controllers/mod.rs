use anyhow::Result;
use kube::Client;
use tokio::{join, spawn};

//mod gateway;
mod gateway_class;
mod gateway_class_parameters;
mod state;

pub async fn run() -> Result<()> {
    let client = Client::try_default().await?;

    let (gateway_class_controller, gateway_class_state_rx) =
        gateway_class::controller(&client).await?;
    let (gateway_class_parameters_controller, mut gateway_class_parameters_state_rx) =
        gateway_class_parameters::controller(&client, &gateway_class_state_rx).await?;

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
        x
    );

    Ok(())
}
