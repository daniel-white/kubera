mod cli;
mod controllers;
pub mod objects;

pub mod ipc;

use crate::controllers::ControllerRunParamsBuilder;
use crate::ipc::{spawn_ipc_service, IpcServiceConfiguration};
use anyhow::Result;
use clap::Parser;
use cli::Cli;
use controllers::run;
use kubera_core::config::logging::init_logging;
use std::sync::Arc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    #[cfg(debug_assertions)]
    unsafe {
        backtrace_on_stack_overflow::enable()
    };

    init_logging();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let _cli = Cli::parse();

    let ipc_configuration = IpcServiceConfiguration::new_builder()
        .port(_cli.control_service_port())
        .build()?;
    let ipc_services = spawn_ipc_service(ipc_configuration).await?;

    let params = ControllerRunParamsBuilder::default()
        .ipc_services(Arc::new(ipc_services))
        .pod_namespace(_cli.pod_namespace())
        .pod_name(_cli.pod_name())
        .instance_name(_cli.instance_name())
        .build()?;

    run(params).await
}
