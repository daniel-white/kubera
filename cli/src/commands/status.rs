use crate::cli::Cli;
use crate::table_theme::{EmojiFormatter, TableTheme};
use anyhow::{Context, Result};
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use kube::{Api, Client};
use tabled::{Table, Tabled};
use vg_api::v1alpha1::StaticResponseFilter;

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum StatusResourceType {
    #[value(name = "gateway", alias = "gw")]
    Gateway,
    #[value(name = "httproute", alias = "route")]
    HTTPRoute,
    #[value(name = "gatewayclass", alias = "gwc")]
    GatewayClass,
    #[value(name = "staticresponsefilter", alias = "srf")]
    StaticResponseFilter,
    #[value(name = "all")]
    All,
}

impl std::str::FromStr for StatusResourceType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "gateway" | "gw" => Ok(StatusResourceType::Gateway),
            "httproute" | "route" => Ok(StatusResourceType::HTTPRoute),
            "gatewayclass" | "gwc" => Ok(StatusResourceType::GatewayClass),
            "staticresponsefilter" | "srf" => Ok(StatusResourceType::StaticResponseFilter),
            "all" => Ok(StatusResourceType::All),
            _ => Err(anyhow::anyhow!("Invalid resource type: {}", s)),
        }
    }
}

#[derive(Tabled)]
struct GatewayStatusRow {
    name: String,
    namespace: String,
    accepted: String,
    programmed: String,
    addresses: String,
    listeners: String,
    age: String,
}

#[derive(Tabled)]
struct HTTPRouteStatusRow {
    name: String,
    namespace: String,
    parents: String,
    accepted: String,
    resolved_refs: String,
    reason: String,
    age: String,
}

#[derive(Tabled)]
struct GatewayClassStatusRow {
    name: String,
    accepted: String,
    controller: String,
    description: String,
    age: String,
}

#[derive(Tabled)]
struct StaticResponseFilterStatusRow {
    name: String,
    namespace: String,
    accepted: String,
    ready: String,
    attached: String,
    attached_routes: String,
    status_code: String,
    age: String,
}

pub async fn handle_status_command(
    client: &Client,
    resource_type: &StatusResourceType,
    name: Option<&str>,
    cli: &Cli,
) -> Result<()> {
    match resource_type {
        StatusResourceType::Gateway => show_gateway_status(client, name, cli).await?,
        StatusResourceType::HTTPRoute => show_httproute_status(client, name, cli).await?,
        StatusResourceType::GatewayClass => show_gatewayclass_status(client, name, cli).await?,
        StatusResourceType::StaticResponseFilter => {
            show_staticresponsefilter_status(client, name, cli).await?
        }
        StatusResourceType::All => {
            show_gateway_status(client, name, cli).await?;
            println!();
            show_httproute_status(client, name, cli).await?;
            println!();
            show_gatewayclass_status(client, name, cli).await?;
            println!();
            show_staticresponsefilter_status(client, name, cli).await?;
        }
    }
    Ok(())
}

async fn show_gateway_status(client: &Client, name: Option<&str>, cli: &Cli) -> Result<()> {
    let namespace = cli.namespace.as_deref();

    let gateways = if let Some(ns) = namespace {
        let api: Api<Gateway> = Api::namespaced(client.clone(), ns);
        if let Some(_name) = name {
            vec![api.get(_name).await.context("Failed to get Gateway")?]
        } else {
            api.list(&Default::default())
                .await
                .context("Failed to list Gateways")?
                .items
        }
    } else {
        let api: Api<Gateway> = Api::all(client.clone());
        if let Some(_name) = name {
            // For cluster-wide search, we need to search all namespaces
            return Err(anyhow::anyhow!(
                "Specify namespace when getting a specific Gateway"
            ));
        } else {
            api.list(&Default::default())
                .await
                .context("Failed to list Gateways")?
                .items
        }
    };

    if gateways.is_empty() {
        println!("No Gateways found.");
        return Ok(());
    }

    let rows: Vec<GatewayStatusRow> = gateways
        .into_iter()
        .map(|gw| {
            let name = gw.metadata.name.unwrap_or_default();
            let namespace = gw.metadata.namespace.unwrap_or_default();
            let age = format_age(gw.metadata.creation_timestamp.as_ref());

            let (accepted, programmed, addresses, listeners) = if let Some(status) = &gw.status {
                let accepted = status
                    .conditions
                    .as_ref()
                    .and_then(|conditions| conditions.iter().find(|c| c.type_ == "Accepted"))
                    .map(|c| if c.status == "True" { "True" } else { "False" })
                    .unwrap_or("Unknown")
                    .to_string();

                let programmed = status
                    .conditions
                    .as_ref()
                    .and_then(|conditions| conditions.iter().find(|c| c.type_ == "Programmed"))
                    .map(|c| if c.status == "True" { "True" } else { "False" })
                    .unwrap_or("Unknown")
                    .to_string();

                let addresses = status
                    .addresses
                    .as_ref()
                    .map(|addrs| {
                        addrs
                            .iter()
                            .map(|addr| addr.value.clone())
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_else(|| "None".to_string());

                let listeners = status
                    .listeners
                    .as_ref()
                    .map(|listeners| listeners.len().to_string())
                    .unwrap_or_else(|| "0".to_string());

                (accepted, programmed, addresses, listeners)
            } else {
                (
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "None".to_string(),
                    "0".to_string(),
                )
            };

            GatewayStatusRow {
                name,
                namespace,
                accepted,
                programmed,
                addresses,
                listeners,
                age,
            }
        })
        .collect();

    let mut table = if cli.emoji {
        TableTheme::apply_status_with_emoji(Table::new(rows))
    } else {
        TableTheme::apply_status(Table::new(rows))
    };

    // Apply emoji formatting to specific columns when enabled
    if cli.emoji {
        // Accepted column (index 2), Programmed column (index 3)
        table = EmojiFormatter::apply_to_column(table, 2); // Accepted (True/False)
        table = EmojiFormatter::apply_to_column(table, 3); // Programmed (True/False)
    }

    println!("{}", table);

    Ok(())
}

async fn show_httproute_status(client: &Client, name: Option<&str>, cli: &Cli) -> Result<()> {
    let namespace = cli.namespace.as_deref();

    let routes = if let Some(ns) = namespace {
        let api: Api<HTTPRoute> = Api::namespaced(client.clone(), ns);
        if let Some(name) = name {
            vec![api.get(name).await.context("Failed to get HTTPRoute")?]
        } else {
            api.list(&Default::default())
                .await
                .context("Failed to list HTTPRoutes")?
                .items
        }
    } else {
        let api: Api<HTTPRoute> = Api::all(client.clone());
        if let Some(_name) = name {
            return Err(anyhow::anyhow!(
                "Specify namespace when getting a specific HTTPRoute"
            ));
        } else {
            api.list(&Default::default())
                .await
                .context("Failed to list HTTPRoutes")?
                .items
        }
    };

    if routes.is_empty() {
        println!("No HTTPRoutes found.");
        return Ok(());
    }

    let rows: Vec<HTTPRouteStatusRow> = routes
        .into_iter()
        .map(|route| {
            let name = route.metadata.name.unwrap_or_default();
            let namespace = route.metadata.namespace.unwrap_or_default();
            let age = format_age(route.metadata.creation_timestamp.as_ref());

            let (parents, accepted, resolved_refs, reason) = if let Some(status) = &route.status {
                let parent_count = status.parents.len();
                let parents = parent_count.to_string();

                // Get the overall status from the first parent (or aggregate if multiple)
                let (accepted, resolved_refs, reason) = if let Some(parent_status) =
                    status.parents.first()
                {
                    let empty_conditions = Vec::new();
                    let conditions = parent_status
                        .conditions
                        .as_ref()
                        .unwrap_or(&empty_conditions);

                    let accepted_condition = conditions.iter().find(|c| c.type_ == "Accepted");
                    let resolved_condition = conditions.iter().find(|c| c.type_ == "ResolvedRefs");

                    let accepted = accepted_condition
                        .map(|c| if c.status == "True" { "True" } else { "False" })
                        .unwrap_or("Unknown")
                        .to_string();

                    let resolved_refs = resolved_condition
                        .map(|c| if c.status == "True" { "True" } else { "False" })
                        .unwrap_or("Unknown")
                        .to_string();

                    let reason = match accepted_condition {
                        Some(c) if c.status != "True" => c.reason.clone(),
                        _ => "Accepted".to_string(),
                    };

                    (accepted, resolved_refs, reason)
                } else {
                    (
                        "Unknown".to_string(),
                        "Unknown".to_string(),
                        "No parents".to_string(),
                    )
                };

                (parents, accepted, resolved_refs, reason)
            } else {
                (
                    "0".to_string(),
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "No status".to_string(),
                )
            };

            HTTPRouteStatusRow {
                name,
                namespace,
                parents,
                accepted,
                resolved_refs,
                reason,
                age,
            }
        })
        .collect();

    let mut table = if cli.emoji {
        TableTheme::apply_status_with_emoji(Table::new(rows))
    } else {
        TableTheme::apply_status(Table::new(rows))
    };

    // Apply emoji formatting to specific columns when enabled
    if cli.emoji {
        // Accepted column (index 3), Resolved Refs column (index 4)
        table = EmojiFormatter::apply_to_column(table, 3); // Accepted (True/False)
        table = EmojiFormatter::apply_to_column(table, 4); // Resolved Refs (True/False)
    }

    println!("{}", table);

    Ok(())
}

async fn show_gatewayclass_status(client: &Client, name: Option<&str>, cli: &Cli) -> Result<()> {
    // GatewayClass is cluster-scoped, so we don't use namespace
    let api: Api<GatewayClass> = Api::all(client.clone());

    let gateway_classes = if let Some(name) = name {
        vec![api.get(name).await.context("Failed to get GatewayClass")?]
    } else {
        api.list(&Default::default())
            .await
            .context("Failed to list GatewayClasses")?
            .items
    };

    if gateway_classes.is_empty() {
        println!("No GatewayClasses found.");
        return Ok(());
    }

    let rows: Vec<GatewayClassStatusRow> = gateway_classes
        .into_iter()
        .map(|gwc| {
            let name = gwc.metadata.name.unwrap_or_default();
            let accepted = gwc
                .status
                .as_ref()
                .and_then(|status| {
                    status.conditions.as_ref().and_then(|conditions| {
                        conditions.iter().find(|c| c.type_ == "Accepted").map(|c| {
                            if c.status == "True" {
                                "True"
                            } else {
                                "False"
                            }
                        })
                    })
                })
                .unwrap_or("Unknown")
                .to_string();
            let controller = gwc.spec.controller_name.clone();
            let description = gwc
                .metadata
                .annotations
                .as_ref()
                .and_then(|annotations| annotations.get("description"))
                .cloned()
                .unwrap_or_default();
            let age = format_age(gwc.metadata.creation_timestamp.as_ref());

            GatewayClassStatusRow {
                name,
                accepted,
                controller,
                description,
                age,
            }
        })
        .collect();

    let mut table = if cli.emoji {
        TableTheme::apply_status_with_emoji(Table::new(rows))
    } else {
        TableTheme::apply_status(Table::new(rows))
    };

    // Apply emoji formatting to specific columns when enabled
    if cli.emoji {
        // Accepted column (index 1)
        table = EmojiFormatter::apply_to_column(table, 1); // Accepted (True/False)
    }

    println!("{}", table);

    Ok(())
}

async fn show_staticresponsefilter_status(
    client: &Client,
    name: Option<&str>,
    cli: &Cli,
) -> Result<()> {
    let namespace = cli.namespace.as_deref();

    let filters = if let Some(ns) = namespace {
        let api: Api<StaticResponseFilter> = Api::namespaced(client.clone(), ns);
        if let Some(name) = name {
            vec![api
                .get(name)
                .await
                .context("Failed to get StaticResponseFilter")?]
        } else {
            api.list(&Default::default())
                .await
                .context("Failed to list StaticResponseFilters")?
                .items
        }
    } else {
        let api: Api<StaticResponseFilter> = Api::all(client.clone());
        if let Some(name) = name {
            return Err(anyhow::anyhow!(
                "Specify namespace when getting a specific StaticResponseFilter"
            ));
        } else {
            api.list(&Default::default())
                .await
                .context("Failed to list StaticResponseFilters")?
                .items
        }
    };

    if filters.is_empty() {
        println!("No StaticResponseFilters found.");
        return Ok(());
    }

    let rows: Vec<StaticResponseFilterStatusRow> = filters
        .into_iter()
        .map(|srf| {
            let name = srf.metadata.name.unwrap_or_default();
            let namespace = srf.metadata.namespace.unwrap_or_default();
            let age = format_age(srf.metadata.creation_timestamp.as_ref());
            let spec_status_code = srf.spec.status_code.to_string();

            let (accepted, ready, attached, attached_routes) = if let Some(status) = &srf.status {
                let accepted = status
                    .conditions
                    .as_ref()
                    .and_then(|conditions| conditions.iter().find(|c| c.type_ == "Accepted"))
                    .map(|c| if c.status == "True" { "True" } else { "False" })
                    .unwrap_or("Unknown")
                    .to_string();

                let ready = status
                    .conditions
                    .as_ref()
                    .and_then(|conditions| conditions.iter().find(|c| c.type_ == "Ready"))
                    .map(|c| if c.status == "True" { "True" } else { "False" })
                    .unwrap_or("Unknown")
                    .to_string();

                let attached = status
                    .conditions
                    .as_ref()
                    .and_then(|conditions| conditions.iter().find(|c| c.type_ == "Attached"))
                    .map(|c| if c.status == "True" { "True" } else { "False" })
                    .unwrap_or("Unknown")
                    .to_string();

                let attached_routes = status.attached_routes.to_string();

                (accepted, ready, attached, attached_routes)
            } else {
                (
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "0".to_string(),
                )
            };

            StaticResponseFilterStatusRow {
                name,
                namespace,
                accepted,
                ready,
                attached,
                attached_routes,
                status_code: spec_status_code,
                age,
            }
        })
        .collect();

    let mut table = if cli.emoji {
        TableTheme::apply_status_with_emoji(Table::new(rows))
    } else {
        TableTheme::apply_status(Table::new(rows))
    };

    // Apply emoji formatting to specific columns when enabled
    if cli.emoji {
        // Accepted (index 2), Ready (index 3), Attached (index 4), Status Code (index 6)
        table = EmojiFormatter::apply_to_column(table, 2); // Accepted (True/False)
        table = EmojiFormatter::apply_to_column(table, 3); // Ready (True/False)
        table = EmojiFormatter::apply_to_column(table, 4); // Attached (True/False)
        table = EmojiFormatter::apply_to_column(table, 6); // Status Code (HTTP codes)
    }

    println!("{}", table);

    Ok(())
}

fn format_age(
    creation_timestamp: Option<&k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>,
) -> String {
    match creation_timestamp {
        Some(timestamp) => {
            let now = chrono::Utc::now();
            let created = timestamp.0;
            let duration = now.signed_duration_since(created);

            if duration.num_days() > 0 {
                format!("{}d", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{}h", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{}m", duration.num_minutes())
            } else {
                format!("{}s", duration.num_seconds().max(0))
            }
        }
        None => "Unknown".to_string(),
    }
}
