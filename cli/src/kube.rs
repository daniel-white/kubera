use anyhow::{Context, Result};
use gateway_api::gateways::Gateway;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{Api, Client, Config};

// Re-export types from commands for backward compatibility
pub use crate::commands::context::ContextInfo;

/// Create a Kubernetes client using kube-conf for proper context handling
pub async fn create_kube_client(kubeconfig_path: Option<&str>) -> Result<Client> {
    let config = if let Some(_path) = kubeconfig_path {
        // For now, use default configuration - kubeconfig path loading can be enhanced later
        Config::infer().await?
    } else {
        // Use default configuration (respects KUBECONFIG env var, then ~/.kube/config)
        Config::infer().await?
    };

    Client::try_from(config).context("Failed to create Kubernetes client")
}

/// Get gateway resources from the cluster
pub async fn get_gateways(
    client: &Client,
    namespace: Option<&str>,
    name: Option<&str>,
    all_namespaces: bool,
    selector: Option<&str>,
) -> Result<Vec<Gateway>> {
    let api: Api<Gateway> = if all_namespaces || namespace.is_none() {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), namespace.unwrap())
    };

    let mut list_params = kube::api::ListParams::default();
    if let Some(sel) = selector {
        list_params = list_params.labels(sel);
    }

    let gateways = api.list(&list_params).await?;

    let mut results = Vec::new();
    for gateway in gateways.items {
        if let Some(target_name) = name {
            if gateway.metadata.name.as_ref() != Some(&target_name.to_string()) {
                continue;
            }
        }

        results.push(gateway);
    }

    Ok(results)
}

/// Get pods associated with gateway instances
pub async fn get_gateway_pods(
    client: &Client,
    namespace: Option<&str>,
    gateway: Option<&str>,
    all_namespaces: bool,
) -> Result<Vec<Pod>> {
    let api: Api<Pod> = if all_namespaces || namespace.is_none() {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), namespace.unwrap())
    };

    let mut list_params = kube::api::ListParams::default();

    // If gateway is specified, filter by gateway label
    if let Some(gw) = gateway {
        list_params = list_params.labels(&format!("gateway.networking.k8s.io/gateway={}", gw));
    } else {
        // Default to vale-gateway components
        list_params = list_params.labels("app.kubernetes.io/part-of=vale-gateway");
    }

    let pods = api.list(&list_params).await?;

    Ok(pods.items)
}

/// Get services associated with gateway instances
pub async fn get_gateway_services(
    client: &Client,
    namespace: Option<&str>,
    gateway: Option<&str>,
    all_namespaces: bool,
) -> Result<Vec<Service>> {
    let api: Api<Service> = if all_namespaces || namespace.is_none() {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), namespace.unwrap())
    };

    let mut list_params = kube::api::ListParams::default();

    if let Some(gw) = gateway {
        list_params = list_params.labels(&format!("gateway.networking.k8s.io/gateway={}", gw));
    } else {
        list_params = list_params.labels("app.kubernetes.io/part-of=vale-gateway");
    }

    let services = api.list(&list_params).await?;

    Ok(services.items)
}

/// Get deployments associated with gateway instances
pub async fn get_gateway_deployments(
    client: &Client,
    namespace: Option<&str>,
    gateway: Option<&str>,
    all_namespaces: bool,
) -> Result<Vec<Deployment>> {
    let api: Api<Deployment> = if all_namespaces || namespace.is_none() {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), namespace.unwrap())
    };

    let mut list_params = kube::api::ListParams::default();

    if let Some(gw) = gateway {
        list_params = list_params.labels(&format!("gateway.networking.k8s.io/gateway={}", gw));
    } else {
        list_params = list_params.labels("app.kubernetes.io/part-of=vale-gateway");
    }

    let deployments = api.list(&list_params).await?;

    Ok(deployments.items)
}

/// Get current Kubernetes context information
#[allow(dead_code)] // TODO: Will be used when context command is fully implemented
pub async fn get_current_context_info(_kubeconfig_path: Option<&str>) -> Result<ContextInfo> {
    // Re-export the function from context command
    crate::commands::context::get_current_context_info(_kubeconfig_path).await
}

/// Get effective namespace - either from CLI arg or default
#[allow(dead_code)] // TODO: Will be used for namespace resolution
pub fn get_effective_namespace(
    cli_namespace: Option<&str>,
    _client: &Client,
    _kubeconfig: Option<&str>,
) -> String {
    cli_namespace.unwrap_or("default").to_string()
}

/// Parse duration string to seconds
#[allow(dead_code)] // TODO: Will be used for log duration parsing
pub fn parse_duration_to_seconds(duration: &str) -> Result<i64> {
    // Simple implementation - just parse as seconds for now
    // TODO: Implement proper duration parsing (e.g., "5m", "1h")
    Ok(duration.parse().unwrap_or(0))
}
