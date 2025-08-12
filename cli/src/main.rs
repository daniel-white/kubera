mod cli;
mod commands;
mod kube;
mod table_theme;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use commands::handle_command;
use kube::create_kube_client;
use vg_core::crypto::init_crypto;

#[tokio::main]
async fn main() -> Result<()> {
    init_crypto();

    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    } else {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    match create_kube_client(cli.kubeconfig.as_deref()).await {
        Ok(client) => {
            handle_command(&client, &cli).await?;
        }
        Err(e) => {
            eprintln!("Failed to create Kubernetes client: {}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}
