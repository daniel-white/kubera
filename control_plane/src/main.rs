mod cli;
mod controllers;
pub mod objects;

pub mod ipc;

use crate::controllers::ControllerRunParamsBuilder;
use crate::ipc::{IpcServiceConfiguration, spawn_ipc_service};
use anyhow::Result;
use clap::Parser;
use cli::Cli;
use controllers::run;
use kubera_core::crypto::init_crypto;
use kubera_core::instrumentation::init_instrumentation;
use std::sync::Arc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    init_crypto();
    init_instrumentation();

    let args = Cli::parse();

    let ipc_configuration = IpcServiceConfiguration::new_builder()
        .port(args.port())
        .build()?;
    let ipc_services = spawn_ipc_service(ipc_configuration).await?;

    let params = ControllerRunParamsBuilder::default()
        .ipc_services(Arc::new(ipc_services))
        .pod_namespace(args.pod_namespace())
        .pod_name(args.pod_name())
        .instance_name(args.instance_name())
        .build()?;

    run(params).await
}
