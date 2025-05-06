mod api;
mod controllers;

use api::write_crds;
use clap::{Parser, Subcommand};
use controllers::run_controllers;
use structured_logger::Builder;
use structured_logger::async_json::new_writer;

#[derive(Parser)]
#[command(name = "kubera-controller")]
#[command(about = "A Kubernetes controller for Kubera", long_about = None)]
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

fn main() {
    /*    Builder::with_level("info")
    .with_target_writer("*", new_writer(s()))
    .init(); */

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            run_controllers();
        }
        Commands::WriteCrds { output_path } => {
            write_crds(output_path.as_deref()).expect("Failed to write CRDs");
        }
    }
}
