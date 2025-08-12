use anyhow::Result;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::LogParams, Api, Client};
use tracing::warn;

use crate::cli::Cli;
use crate::kube::get_gateway_pods;

/// Get effective namespace - either from CLI arg or default
fn get_effective_namespace(
    cli_namespace: Option<&str>,
    _client: &Client,
    _kubeconfig: Option<&str>,
) -> String {
    cli_namespace.unwrap_or("default").to_string()
}

/// Parse duration string to seconds
fn parse_duration_to_seconds(duration: &str) -> Result<i64> {
    // Simple implementation - just parse as seconds for now
    // TODO: Implement proper duration parsing (e.g., "5m", "1h")
    Ok(duration.parse().unwrap_or(0))
}

/// Handle logs command
pub async fn handle_logs_command(
    client: &Client,
    name: &str,
    follow: bool,
    tail: Option<i64>,
    since: Option<&str>,
    container: Option<&str>,
    cli: &Cli,
) -> Result<()> {
    let namespace =
        get_effective_namespace(cli.namespace.as_deref(), client, cli.kubeconfig.as_deref());

    let pods = get_gateway_pods(client, Some(&namespace), Some(name), false).await?;

    if pods.is_empty() {
        warn!("No pods found for gateway: {}", name);
        return Ok(());
    }

    for pod in pods {
        let pod_name = pod.metadata.name.as_ref().unwrap();
        let pod_namespace = pod.metadata.namespace.as_ref().unwrap();

        println!("=== Logs from pod: {} ===", pod_name);

        let pods_api: Api<Pod> = Api::namespaced(client.clone(), pod_namespace);

        let log_params = LogParams {
            container: container.map(|s| s.to_string()),
            follow,
            tail_lines: tail,
            since_seconds: since.and_then(|s| parse_duration_to_seconds(s).ok()),
            ..Default::default()
        };

        let logs = pods_api.logs(pod_name, &log_params).await?;
        println!("{}", logs);
    }

    Ok(())
}
