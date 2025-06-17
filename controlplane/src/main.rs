mod cli;
mod controllers;
pub mod objects;

pub mod ipc;

use crate::ipc::{spawn_ipc_service, IpcServiceConfiguration};
use anyhow::Result;
use clap::{Parser, Subcommand};
use cli::Cli;
use controllers::run;
use kubera_core::config::logging::init_logging;
use kubera_core::net::Port;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    #[cfg(debug_assertions)]
    unsafe {
        backtrace_on_stack_overflow::enable()
    };

    init_logging();

    let cli = Cli::parse();

    let ipc_configuration = IpcServiceConfiguration::new_builder()
        .port(Port::new(8000))
        .build()?;
    let ipc_services = spawn_ipc_service(ipc_configuration).await?;

    run(ipc_services).await
}
