use anyhow::Result;
use chrono::{DateTime, Utc};
use gateway_api::gateways::Gateway;
use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{Pod, Service},
};
use kube::Client;
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

use crate::cli::{Cli, GetResource, OutputFormat};
use crate::kube::{get_gateway_deployments, get_gateway_pods, get_gateway_services, get_gateways};
use crate::table_theme::{EmojiFormatter, TableTheme};

fn themed_table<T: tabled::Tabled>(rows: Vec<T>, format: &OutputFormat) -> Table {
    match format {
        OutputFormat::Table => TableTheme::apply_default(Table::new(rows)),
        OutputFormat::TableEmoji => TableTheme::apply_default_with_emoji(Table::new(rows)),
        OutputFormat::Wide => TableTheme::apply_wide(Table::new(rows)),
        OutputFormat::WideEmoji => TableTheme::apply_wide_with_emoji(Table::new(rows)),
        OutputFormat::Kubectl => TableTheme::apply_kubectl(Table::new(rows)),
        _ => TableTheme::apply_default(Table::new(rows)),
    }
}

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
        let spec = gateway.spec;
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

// Helper functions for formatting complex fields
fn format_age_from_datetime(timestamp: DateTime<Utc>) -> String {
    let duration = Utc::now().signed_duration_since(timestamp);
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

fn format_age_from_option(age: Option<DateTime<Utc>>) -> String {
    match age {
        Some(timestamp) => format_age_from_datetime(timestamp),
        None => "<unknown>".to_string(),
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

        let external_ips = spec.external_ips.unwrap_or_default();

        let ports = spec
            .ports
            .unwrap_or_default()
            .into_iter()
            .map(|port| {
                let target_port = match port.target_port {
                    Some(target) => match target {
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(i) => {
                            i.to_string()
                        }
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(s) => s,
                    },
                    None => port.port.to_string(),
                };
                format!(
                    "{}:{}/{}",
                    port.protocol.unwrap_or_default().to_lowercase(),
                    port.port,
                    target_port
                )
            })
            .collect();

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

// Table row structures for consistent display
#[derive(Tabled)]
struct GatewayTableRow {
    name: String,
    namespace: String,
    class: String,
    address: String,
    age: String,
}

#[derive(Tabled)]
struct PodTableRow {
    name: String,
    namespace: String,
    ready: String,
    status: String,
    restarts: i32,
    age: String,
}

#[derive(Tabled)]
struct PodWideTableRow {
    name: String,
    namespace: String,
    ready: String,
    status: String,
    restarts: i32,
    node: String,
    age: String,
}

#[derive(Tabled)]
struct ServiceTableRow {
    name: String,
    namespace: String,
    r#type: String,
    cluster_ip: String,
    ports: String,
    age: String,
}

#[derive(Tabled)]
struct ServiceWideTableRow {
    name: String,
    namespace: String,
    r#type: String,
    cluster_ip: String,
    external_ip: String,
    ports: String,
    age: String,
}

#[derive(Tabled)]
struct DeploymentTableRow {
    name: String,
    namespace: String,
    ready: String,
    up_to_date: i32,
    available: i32,
    age: String,
}

/// Output formatting functions
fn output_gateways(gateways: &[GatewayInfo], format: &OutputFormat, _cli: &Cli) -> Result<()> {
    match format {
        OutputFormat::Table
        | OutputFormat::TableEmoji
        | OutputFormat::Wide
        | OutputFormat::WideEmoji
        | OutputFormat::Kubectl => {
            let rows: Vec<GatewayTableRow> = gateways
                .iter()
                .map(|gateway| {
                    let address = gateway
                        .addresses
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "<none>".to_string());
                    let age = format_age_from_option(gateway.age);

                    GatewayTableRow {
                        name: gateway.name.clone(),
                        namespace: gateway.namespace.clone(),
                        class: gateway.gateway_class.clone(),
                        address,
                        age,
                    }
                })
                .collect();

            let mut table = themed_table(rows, format);
            // Apply emoji formatting to relevant columns for emoji formats
            if matches!(format, OutputFormat::TableEmoji | OutputFormat::WideEmoji) {
                table = EmojiFormatter::apply_to_column(table, 3); // Address column
            }
            println!("{}", table);
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

fn output_pods(pods: &[PodInfo], format: &OutputFormat, _cli: &Cli) -> Result<()> {
    match format {
        OutputFormat::Table => {
            let rows: Vec<PodTableRow> = pods
                .iter()
                .map(|pod| {
                    let age = format_age_from_option(pod.age);

                    PodTableRow {
                        name: pod.name.clone(),
                        namespace: pod.namespace.clone(),
                        ready: pod.ready.clone(),
                        status: pod.status.clone(),
                        restarts: pod.restarts,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_default(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::TableEmoji => {
            let rows: Vec<PodTableRow> = pods
                .iter()
                .map(|pod| {
                    let age = format_age_from_option(pod.age);

                    PodTableRow {
                        name: pod.name.clone(),
                        namespace: pod.namespace.clone(),
                        ready: pod.ready.clone(),
                        status: pod.status.clone(),
                        restarts: pod.restarts,
                        age,
                    }
                })
                .collect();

            let mut table = TableTheme::apply_default_with_emoji(Table::new(rows));
            // Apply emoji formatting to relevant columns
            table = EmojiFormatter::apply_to_column(table, 2); // Ready (e.g., "1/1")
            table = EmojiFormatter::apply_to_column(table, 3); // Status (e.g., "Running")
            table = EmojiFormatter::apply_to_column(table, 4); // Restarts (numeric)
            println!("{}", table);
        }
        OutputFormat::Kubectl => {
            let rows: Vec<PodTableRow> = pods
                .iter()
                .map(|pod| {
                    let age = format_age_from_option(pod.age);

                    PodTableRow {
                        name: pod.name.clone(),
                        namespace: pod.namespace.clone(),
                        ready: pod.ready.clone(),
                        status: pod.status.clone(),
                        restarts: pod.restarts,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_kubectl(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::Wide | OutputFormat::WideEmoji => {
            let rows: Vec<PodWideTableRow> = pods
                .iter()
                .map(|pod| {
                    let age = format_age_from_option(pod.age);
                    let node = pod.node.as_deref().unwrap_or("<none>").to_string();

                    PodWideTableRow {
                        name: pod.name.clone(),
                        namespace: pod.namespace.clone(),
                        ready: pod.ready.clone(),
                        status: pod.status.clone(),
                        restarts: pod.restarts,
                        node,
                        age,
                    }
                })
                .collect();

            let mut table = if matches!(format, OutputFormat::WideEmoji) {
                TableTheme::apply_wide_with_emoji(Table::new(rows))
            } else {
                TableTheme::apply_wide(Table::new(rows))
            };

            if matches!(format, OutputFormat::WideEmoji) {
                table = EmojiFormatter::apply_to_column(table, 2); // Ready
                table = EmojiFormatter::apply_to_column(table, 3); // Status
                table = EmojiFormatter::apply_to_column(table, 4); // Restarts
            }

            println!("{}", table);
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

fn output_services(services: &[ServiceInfo], format: &OutputFormat, _cli: &Cli) -> Result<()> {
    match format {
        OutputFormat::Table => {
            let rows: Vec<ServiceTableRow> = services
                .iter()
                .map(|service| {
                    let ports = service.ports.join(",");
                    let age = format_age_from_option(service.age);
                    let cluster_ip = service
                        .cluster_ip
                        .as_deref()
                        .unwrap_or("<none>")
                        .to_string();

                    ServiceTableRow {
                        name: service.name.clone(),
                        namespace: service.namespace.clone(),
                        r#type: service.service_type.clone(),
                        cluster_ip,
                        ports,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_default(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::TableEmoji => {
            let rows: Vec<ServiceTableRow> = services
                .iter()
                .map(|service| {
                    let ports = service.ports.join(",");
                    let age = format_age_from_option(service.age);
                    let cluster_ip = service
                        .cluster_ip
                        .as_deref()
                        .unwrap_or("<none>")
                        .to_string();

                    ServiceTableRow {
                        name: service.name.clone(),
                        namespace: service.namespace.clone(),
                        r#type: service.service_type.clone(),
                        cluster_ip,
                        ports,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_default_with_emoji(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::Kubectl => {
            let rows: Vec<ServiceTableRow> = services
                .iter()
                .map(|service| {
                    let ports = service.ports.join(",");
                    let age = format_age_from_option(service.age);
                    let cluster_ip = service
                        .cluster_ip
                        .as_deref()
                        .unwrap_or("<none>")
                        .to_string();

                    ServiceTableRow {
                        name: service.name.clone(),
                        namespace: service.namespace.clone(),
                        r#type: service.service_type.clone(),
                        cluster_ip,
                        ports,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_kubectl(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::Wide | OutputFormat::WideEmoji => {
            let rows: Vec<ServiceWideTableRow> = services
                .iter()
                .map(|service| {
                    let ports = service.ports.join(",");
                    let external_ip = if service.external_ips.is_empty() {
                        "<none>".to_string()
                    } else {
                        service.external_ips.join(",")
                    };
                    let age = format_age_from_option(service.age);
                    let cluster_ip = service
                        .cluster_ip
                        .as_deref()
                        .unwrap_or("<none>")
                        .to_string();

                    ServiceWideTableRow {
                        name: service.name.clone(),
                        namespace: service.namespace.clone(),
                        r#type: service.service_type.clone(),
                        cluster_ip,
                        external_ip,
                        ports,
                        age,
                    }
                })
                .collect();

            let table = if matches!(format, OutputFormat::WideEmoji) {
                TableTheme::apply_wide_with_emoji(Table::new(rows))
            } else {
                TableTheme::apply_wide(Table::new(rows))
            };
            println!("{}", table);
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

fn output_deployments(
    deployments: &[DeploymentInfo],
    format: &OutputFormat,
    _cli: &Cli,
) -> Result<()> {
    match format {
        OutputFormat::Table => {
            let rows: Vec<DeploymentTableRow> = deployments
                .iter()
                .map(|deployment| {
                    let age = format_age_from_option(deployment.age);

                    DeploymentTableRow {
                        name: deployment.name.clone(),
                        namespace: deployment.namespace.clone(),
                        ready: deployment.ready.clone(),
                        up_to_date: deployment.up_to_date,
                        available: deployment.available,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_default(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::TableEmoji => {
            let rows: Vec<DeploymentTableRow> = deployments
                .iter()
                .map(|deployment| {
                    let age = format_age_from_option(deployment.age);

                    DeploymentTableRow {
                        name: deployment.name.clone(),
                        namespace: deployment.namespace.clone(),
                        ready: deployment.ready.clone(),
                        up_to_date: deployment.up_to_date,
                        available: deployment.available,
                        age,
                    }
                })
                .collect();

            let mut table = TableTheme::apply_default_with_emoji(Table::new(rows));
            // Apply emoji formatting to relevant columns
            table = EmojiFormatter::apply_to_column(table, 2); // Ready column
            println!("{}", table);
        }
        OutputFormat::Kubectl => {
            let rows: Vec<DeploymentTableRow> = deployments
                .iter()
                .map(|deployment| {
                    let age = format_age_from_option(deployment.age);

                    DeploymentTableRow {
                        name: deployment.name.clone(),
                        namespace: deployment.namespace.clone(),
                        ready: deployment.ready.clone(),
                        up_to_date: deployment.up_to_date,
                        available: deployment.available,
                        age,
                    }
                })
                .collect();

            let table = TableTheme::apply_kubectl(Table::new(rows));
            println!("{}", table);
        }
        OutputFormat::Wide | OutputFormat::WideEmoji => {
            output_deployments(deployments, &OutputFormat::Table, _cli)?
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(deployments)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(deployments)?);
        }
    }
    Ok(())
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

            output_gateways(&gateway_info, &cli.output, cli)?;
        }
        GetResource::GatewayClasses { name: _name } => {
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

            output_pods(&pod_info, &cli.output, cli)?;
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

            output_services(&service_info, &cli.output, cli)?;
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

            output_deployments(&deployment_info, &cli.output, cli)?;
        }
    }

    Ok(())
}
