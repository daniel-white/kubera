mod api;
mod cli;
mod constants;
mod control_service;
mod controllers;
pub mod objects;

use anyhow::Result;
use api::write_crds;
use clap::{Parser, Subcommand};
use cli::{Cli, Commands};
use controllers::run;
use kubera_core::config::logging::init_logging;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    #[cfg(debug_assertions)]
    unsafe {
        backtrace_on_stack_overflow::enable()
    };

    init_logging();

    let cli = Cli::parse();

    match cli.command().as_ref().unwrap_or(&Commands::Run) {
        Commands::Run => run().await,
        Commands::WriteCrds { output_path } => write_crds(output_path.as_deref()),
    }
}
