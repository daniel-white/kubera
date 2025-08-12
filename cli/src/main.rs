mod cli;
mod commands;
mod kube;
mod models;

use anyhow::{Context, Result};
use clap::Parser;

use cli::Cli;
use commands::handle_command;
use kube::create_kube_client;

#[tokio::main]
async fn main() -> Result<()> {
    vg_core::crypto::init_crypto();

    let cli = Cli::parse();

    // Initialize logging based on verbosity
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    let client = create_kube_client(cli.kubeconfig.as_deref())
        .await
        .context("Failed to create Kubernetes client")?;

    handle_command(&client, &cli).await?;

    Ok(())
}
