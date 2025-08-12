pub mod analyze;
pub mod context;
pub mod describe;
pub mod exec;
pub mod get;
pub mod logs;
pub mod port_forward;
pub mod status;

use anyhow::Result;
use kube::Client;

use crate::cli::{Cli, Commands};

/// Main command dispatcher
pub async fn handle_command(client: &Client, cli: &Cli) -> Result<()> {
    match &cli.command {
        Commands::Get { ref resource } => {
            get::handle_get_command(client, resource, cli).await?;
        }
        Commands::Describe { ref resource } => {
            describe::handle_describe_command(client, resource, cli).await?;
        }
        Commands::Logs {
            ref name,
            follow,
            tail,
            ref since,
            ref container,
        } => {
            logs::handle_logs_command(
                client,
                name,
                *follow,
                Some(*tail),
                since.as_deref(),
                container.as_deref(),
                cli,
            )
            .await?;
        }
        Commands::Analyze {
            ref name,
            config,
            listeners,
            routes,
        } => {
            analyze::handle_analyze_command(
                client,
                name.as_deref(),
                *config,
                *listeners,
                *routes,
                cli,
            )
            .await?;
        }
        Commands::Context => {
            context::handle_context_command(cli).await?;
        }
        Commands::PortForward {
            ref name,
            local_port,
            remote_port,
        } => {
            port_forward::handle_port_forward_command(client, name, *local_port, *remote_port, cli)
                .await?;
        }
        Commands::Exec {
            ref name,
            ref container,
            ref command,
        } => {
            exec::handle_exec_command(client, name, container.as_deref(), command, cli).await?;
        }
        Commands::Status {
            ref resource_type,
            ref name,
        } => {
            status::handle_status_command(client, resource_type, name.as_deref(), cli).await?;
        }
    }

    Ok(())
}
