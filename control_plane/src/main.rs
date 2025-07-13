#![warn(
    clippy::pedantic,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unwrap_used,
    clippy::expect_used
)]
#![allow(
    clippy::needless_pass_by_value,
    clippy::needless_continue,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::struct_field_names
)]

mod cli;
mod controllers;

mod health;
pub mod ipc;
pub mod kubernetes;
mod options;

use crate::controllers::{
    SpawnControllersError, SpawnControllersParams, SpawnControllersParamsBuilderError,
    spawn_controllers,
};
use crate::ipc::{SpawnIpcError, SpawnIpcParameters, SpawnIpcParametersBuilderError, spawn_ipc};
use crate::kubernetes::start_kubernetes_client;
use crate::options::Options;
use clap::Parser;
use cli::Cli;
use kubera_core::crypto::init_crypto;
use kubera_core::instrumentation::init_instrumentation;
use kubera_core::task::Builder as TaskBuilder;
use std::sync::Arc;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum MainError {
    #[error("Failed to build IPC parameters: {0}")]
    SpawnIpcParameters(#[from] SpawnIpcParametersBuilderError),
    #[error("Failed to spawn IPC services: {0}")]
    SpawnIpc(#[from] SpawnIpcError),
    #[error("Failed to build controllers parameters: {0}")]
    SpawnControllersParams(#[from] SpawnControllersParamsBuilderError),
    #[error("Failed to spawn controllers: {0}")]
    SpawnControllers(#[from] SpawnControllersError),
}

#[tokio::main(flavor = "multi_thread")]
#[allow(clippy::expect_used)] // Expect is used here to ensure that the application fails fast if the parameters are invalid
async fn main() -> Result<(), MainError> {
    let args = Cli::parse();
    let options = Arc::new(Options::default());

    init_crypto();
    init_instrumentation();

    let task_builder = TaskBuilder::default();

    let kube_client_rx = start_kubernetes_client(&task_builder);

    // IPC is half 1 - it is what the gateway use to ensure that they have the latest configuration
    let ipc_services = {
        let params = SpawnIpcParameters::new_builder()
            .options(options.clone())
            .port(args.port())
            .kube_client_rx(kube_client_rx.clone())
            .build()
            .inspect_err(|err| {
                error!("Failed to build IPC parameters: {}", err);
            })?;

        spawn_ipc(&task_builder, params)
            .await
            .inspect_err(|err| error!("Failed to spawn IPC services: {}", err))?
    };

    // Controllers are half 2 - they are responsible for ensuring that the configuration is up to date
    // through various controllers
    {
        let params = SpawnControllersParams::new_builder()
            .options(options)
            .kube_client_rx(kube_client_rx)
            .ipc_services(Arc::new(ipc_services))
            .pod_namespace(args.pod_namespace())
            .pod_name(args.pod_name())
            .instance_name(args.instance_name())
            .build()
            .inspect_err(|err| error!("Failed to build controllers parameters: {}", err))?;

        spawn_controllers(&task_builder, params)
            .inspect_err(|err| error!("Failed to spawn controllers: {}", err))?;
    }

    task_builder.join_all().await;

    Ok(())
}
