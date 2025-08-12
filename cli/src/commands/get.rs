use anyhow::Result;
use chrono::{DateTime, Utc};
use gateway_api::gateways::Gateway;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::Client;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::cli::{Cli, GetResource, OutputFormat};
use crate::kube::{get_gateway_deployments, get_gateway_pods, get_gateway_services, get_gateways};

/// Models specific to the get command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayInfo {
    pub name: String,
    pub namespace: String,
    pub gateway_class: String,
    pub addresses: Vec<String>,
    pub listeners: Vec<ListenerInfo>,
    pub status: String,
    pub age: Option<DateTime<Utc>>,
}

impl GatewayInfo {
    pub fn from_gateway(gateway: Gateway) -> Self {
        let metadata = gateway.metadata;
        let spec = &gateway.spec;
        let status = gateway.status.unwrap_or_default();

        let addresses = status
            .addresses
            .unwrap_or_default()
            .into_iter()
            .map(|addr| addr.value)
            .collect();

        let listeners = spec
            .listeners
            .iter()
            .map(ListenerInfo::from_listener)
            .collect();

        Self {
            name: metadata.name.unwrap_or_default(),
            namespace: metadata.namespace.unwrap_or_default(),
            gateway_class: spec.gateway_class_name.clone(),
            addresses,
            listeners,
            status: "Ready".to_string(), // TODO: Parse actual status
            age: metadata.creation_timestamp.map(|ts| ts.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerInfo {
    pub name: String,
    pub port: i32,
    pub protocol: String,
    pub hostname: Option<String>,
}

impl ListenerInfo {
    pub fn from_listener(listener: &gateway_api::gateways::GatewayListeners) -> Self {
        Self {
            name: listener.name.clone(),
            port: listener.port,
            protocol: listener.protocol.clone(),
            hostname: listener.hostname.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub ready: String,
    pub status: String,
    pub restarts: i32,
    pub age: Option<DateTime<Utc>>,
    pub node: Option<String>,
}

impl PodInfo {
    pub fn from_pod(pod: Pod) -> Self {
        let metadata = pod.metadata;
        let spec = pod.spec.unwrap_or_default();
        let status = pod.status.unwrap_or_default();

        let ready_containers = status
            .container_statuses
            .as_ref()
            .map(|statuses| statuses.iter().filter(|s| s.ready).count())
            .unwrap_or(0);

        let total_containers = spec.containers.len();
        let ready = format!("{}/{}", ready_containers, total_containers);

        let restarts = status
            .container_statuses
            .as_ref()
            .map(|statuses| statuses.iter().map(|s| s.restart_count).sum())
            .unwrap_or(0);

        Self {
            name: metadata.name.unwrap_or_default(),
            namespace: metadata.namespace.unwrap_or_default(),
            ready,
            status: status.phase.unwrap_or_default(),
            restarts,
            age: metadata.creation_timestamp.map(|ts| ts.0),
            node: spec.node_name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub namespace: String,
    pub service_type: String,
    pub cluster_ip: Option<String>,
    pub external_ips: Vec<String>,
    pub ports: Vec<String>,
    pub age: Option<DateTime<Utc>>,
}

impl ServiceInfo {
    pub fn from_service(service: Service) -> Self {
        let metadata = service.metadata;
        let spec = service.spec.unwrap_or_default();

        let ports = spec
            .ports
            .unwrap_or_default()
            .into_iter()
            .map(|port| {
                format!(
                    "{}/{}",
                    port.port,
                    port.protocol.unwrap_or_default().to_lowercase()
                )
            })
            .collect();

        let external_ips = spec.external_ips.unwrap_or_default();

        Self {
            name: metadata.name.unwrap_or_default(),
            namespace: metadata.namespace.unwrap_or_default(),
            service_type: spec.type_.unwrap_or_default(),
            cluster_ip: spec.cluster_ip,
            external_ips,
            ports,
            age: metadata.creation_timestamp.map(|ts| ts.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub name: String,
    pub namespace: String,
    pub ready: String,
    pub up_to_date: i32,
    pub available: i32,
    pub age: Option<DateTime<Utc>>,
}

impl DeploymentInfo {
    pub fn from_deployment(deployment: Deployment) -> Self {
        let metadata = deployment.metadata;
        let spec = deployment.spec.unwrap_or_default();
        let status = deployment.status.unwrap_or_default();

        let desired = spec.replicas.unwrap_or(0);
        let ready = status.ready_replicas.unwrap_or(0);
        let ready_str = format!("{}/{}", ready, desired);

        Self {
            name: metadata.name.unwrap_or_default(),
            namespace: metadata.namespace.unwrap_or_default(),
            ready: ready_str,
            up_to_date: status.updated_replicas.unwrap_or(0),
            available: status.available_replicas.unwrap_or(0),
            age: metadata.creation_timestamp.map(|ts| ts.0),
        }
    }
}

/// Output formatting functions
fn output_gateways(gateways: &[GatewayInfo], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!(
                "{:<20} {:<15} {:<20} {:<15} {:<10}",
                "NAME", "NAMESPACE", "CLASS", "ADDRESS", "AGE"
            );
            for gateway in gateways {
                let address_default = "<none>".to_string();
                let address = gateway.addresses.first().unwrap_or(&address_default);
                let age = gateway
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());

                println!(
                    "{:<20} {:<15} {:<20} {:<15} {:<10}",
                    gateway.name, gateway.namespace, gateway.gateway_class, address, age
                );
            }
        }
        OutputFormat::Wide => {
            println!(
                "{:<20} {:<15} {:<20} {:<15} {:<30} {:<10}",
                "NAME", "NAMESPACE", "CLASS", "ADDRESS", "LISTENERS", "AGE"
            );
            for gateway in gateways {
                let address_default = "<none>".to_string();
                let address = gateway.addresses.first().unwrap_or(&address_default);
                let listeners = gateway
                    .listeners
                    .iter()
                    .map(|l| format!("{}:{}", l.name, l.port))
                    .collect::<Vec<_>>()
                    .join(",");
                let age = gateway
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());

                println!(
                    "{:<20} {:<15} {:<20} {:<15} {:<30} {:<10}",
                    gateway.name, gateway.namespace, gateway.gateway_class, address, listeners, age
                );
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(gateways)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(gateways)?);
        }
    }
    Ok(())
}

fn output_pods(pods: &[PodInfo], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!(
                "{:<30} {:<15} {:<8} {:<12} {:<8} {:<10}",
                "NAME", "NAMESPACE", "READY", "STATUS", "RESTARTS", "AGE"
            );
            for pod in pods {
                let age = pod
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());

                println!(
                    "{:<30} {:<15} {:<8} {:<12} {:<8} {:<10}",
                    pod.name, pod.namespace, pod.ready, pod.status, pod.restarts, age
                );
            }
        }
        OutputFormat::Wide => {
            println!(
                "{:<30} {:<15} {:<8} {:<12} {:<8} {:<20} {:<10}",
                "NAME", "NAMESPACE", "READY", "STATUS", "RESTARTS", "NODE", "AGE"
            );
            for pod in pods {
                let age = pod
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());
                let node = pod.node.as_deref().unwrap_or("<none>");

                println!(
                    "{:<30} {:<15} {:<8} {:<12} {:<8} {:<20} {:<10}",
                    pod.name, pod.namespace, pod.ready, pod.status, pod.restarts, node, age
                );
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(pods)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(pods)?);
        }
    }
    Ok(())
}

fn output_services(services: &[ServiceInfo], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!(
                "{:<30} {:<15} {:<12} {:<15} {:<20} {:<10}",
                "NAME", "NAMESPACE", "TYPE", "CLUSTER-IP", "PORTS", "AGE"
            );
            for service in services {
                let ports = service.ports.join(",");
                let age = service
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());
                let cluster_ip = service.cluster_ip.as_deref().unwrap_or("<none>");

                println!(
                    "{:<30} {:<15} {:<12} {:<15} {:<20} {:<10}",
                    service.name, service.namespace, service.service_type, cluster_ip, ports, age
                );
            }
        }
        OutputFormat::Wide => {
            println!(
                "{:<30} {:<15} {:<12} {:<15} {:<15} {:<20} {:<10}",
                "NAME", "NAMESPACE", "TYPE", "CLUSTER-IP", "EXTERNAL-IP", "PORTS", "AGE"
            );
            for service in services {
                let ports = service.ports.join(",");
                let external_ips = if service.external_ips.is_empty() {
                    "<none>".to_string()
                } else {
                    service.external_ips.join(",")
                };
                let age = service
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());
                let cluster_ip = service.cluster_ip.as_deref().unwrap_or("<none>");

                println!(
                    "{:<30} {:<15} {:<12} {:<15} {:<15} {:<20} {:<10}",
                    service.name,
                    service.namespace,
                    service.service_type,
                    cluster_ip,
                    external_ips,
                    ports,
                    age
                );
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(services)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(services)?);
        }
    }
    Ok(())
}

fn output_deployments(deployments: &[DeploymentInfo], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!(
                "{:<30} {:<15} {:<8} {:<10} {:<10} {:<10}",
                "NAME", "NAMESPACE", "READY", "UP-TO-DATE", "AVAILABLE", "AGE"
            );
            for deployment in deployments {
                let age = deployment
                    .age
                    .map(format_age)
                    .unwrap_or_else(|| "<unknown>".to_string());

                println!(
                    "{:<30} {:<15} {:<8} {:<10} {:<10} {:<10}",
                    deployment.name,
                    deployment.namespace,
                    deployment.ready,
                    deployment.up_to_date,
                    deployment.available,
                    age
                );
            }
        }
        OutputFormat::Wide => output_deployments(deployments, &OutputFormat::Table)?,
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(deployments)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(deployments)?);
        }
    }
    Ok(())
}

fn format_age(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_days() > 0 {
        format!("{}d", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m", duration.num_minutes())
    } else {
        format!("{}s", duration.num_seconds())
    }
}

/// Handle all get subcommands
pub async fn handle_get_command(client: &Client, resource: &GetResource, cli: &Cli) -> Result<()> {
    match resource {
        GetResource::Gateways {
            name,
            all_namespaces,
            selector,
        } => {
            let gateways = get_gateways(
                client,
                cli.namespace.as_deref(),
                name.as_deref(),
                *all_namespaces,
                selector.as_deref(),
            )
            .await?;

            let gateway_info: Vec<GatewayInfo> = gateways
                .into_iter()
                .map(GatewayInfo::from_gateway)
                .collect();

            output_gateways(&gateway_info, &cli.output)?;
        }
        GetResource::GatewayClasses { name: _name } => {
            warn!("Gateway classes listing not yet implemented");
            // TODO: Implement gateway class discovery
        }
        GetResource::Pods {
            gateway,
            all_namespaces,
        } => {
            let pods = get_gateway_pods(
                client,
                cli.namespace.as_deref(),
                gateway.as_deref(),
                *all_namespaces,
            )
            .await?;

            let pod_info: Vec<PodInfo> = pods.into_iter().map(PodInfo::from_pod).collect();

            output_pods(&pod_info, &cli.output)?;
        }
        GetResource::Services {
            gateway,
            all_namespaces,
        } => {
            let services = get_gateway_services(
                client,
                cli.namespace.as_deref(),
                gateway.as_deref(),
                *all_namespaces,
            )
            .await?;

            let service_info: Vec<ServiceInfo> = services
                .into_iter()
                .map(ServiceInfo::from_service)
                .collect();

            output_services(&service_info, &cli.output)?;
        }
        GetResource::Deployments {
            gateway,
            all_namespaces,
        } => {
            let deployments = get_gateway_deployments(
                client,
                cli.namespace.as_deref(),
                gateway.as_deref(),
                *all_namespaces,
            )
            .await?;

            let deployment_info: Vec<DeploymentInfo> = deployments
                .into_iter()
                .map(DeploymentInfo::from_deployment)
                .collect();

            output_deployments(&deployment_info, &cli.output)?;
        }
    }

    Ok(())
}
