mod api;
mod constants;
mod controllers;
mod sync;

use anyhow::Result;
use api::write_crds;
use clap::{Parser, Subcommand};
use controllers::run;

#[derive(Parser)]
#[command(name = "kubera-controlplane")]
#[command(about = "A Kubernetes control plane for Kubera", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run,
    WriteCrds {
        #[arg(short, long)]
        output_path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env_or_str("debug")?.start()?;

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => run().await,
        Commands::WriteCrds { output_path } => write_crds(output_path.as_deref()),
    }
}
