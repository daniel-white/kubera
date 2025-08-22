use crate::kubernetes::objects::Objects;
use crate::kubernetes::KubeClientCell;
use anyhow::{Context, Result};
use gateway_api::apis::standard::httproutes::HTTPRoute;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::chrono::Utc;
use kube::api::PostParams;
use kube::{Api, Client};
use std::ops::Deref;
use tracing::{debug, info, info_span, warn, Instrument};
use vg_api::v1alpha1::{
    StaticResponseFilter, StaticResponseFilterConditionReason, StaticResponseFilterConditionType,
    StaticResponseFilterStatus,
};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, ReadyState};

/// Controller for managing `StaticResponseFilter` status updates
pub fn sync_static_response_filter_status(
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    static_response_filters_rx: &Receiver<Objects<StaticResponseFilter>>,
    http_routes_rx: &Receiver<Objects<HTTPRoute>>,
) {
    let kube_client_rx = kube_client_rx.clone();
    let static_response_filters_rx = static_response_filters_rx.clone();
    let http_routes_rx = http_routes_rx.clone();

    task_builder
        .new_task(stringify!(sync_static_response_filter_status))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((kube_client, static_filters, http_routes)) =
                    await_ready!(kube_client_rx, static_response_filters_rx, http_routes_rx)
                {
                    info!("Syncing status for StaticResponseFilters");

                    // Iterate through all static response filters
                    for (filter_ref, _, filter) in static_filters.iter() {
                        debug!("Processing StaticResponseFilter: {}", filter_ref);

                        let attached_routes = count_attached_routes(&filter, http_routes);
                        let status = create_filter_status(&filter.spec, attached_routes);

                        if let Err(e) =
                            update_filter_status(kube_client.deref().clone(), &filter, status).await
                        {
                            warn!(
                                "Failed to update status for StaticResponseFilter {}: {}",
                                filter_ref, e
                            );
                        }
                    }
                }

                vg_core::continue_on!(
                    static_response_filters_rx.changed(),
                    http_routes_rx.changed(),
                    kube_client_rx.changed()
                );
            }
        });
}

/// Count how many routes are using this static response filter
fn count_attached_routes(filter: &StaticResponseFilter, http_routes: &Objects<HTTPRoute>) -> i32 {
    let default_name = String::new();
    let default_namespace = String::new();
    let filter_name = filter.metadata.name.as_ref().unwrap_or(&default_name);
    let filter_namespace = filter
        .metadata
        .namespace
        .as_ref()
        .unwrap_or(&default_namespace);

    let mut count = 0;
    for (_, _, route) in http_routes.iter() {
        if is_filter_attached_to_route(filter_name, filter_namespace, &route) {
            count += 1;
        }
    }
    count
}

/// Check if a static response filter is attached to a specific HTTP route
fn is_filter_attached_to_route(
    filter_name: &str,
    filter_namespace: &str,
    route: &HTTPRoute,
) -> bool {
    if let Some(rules) = &route.spec.rules {
        for rule in rules {
            if let Some(filters) = &rule.filters {
                for filter in filters {
                    if let Some(extension_ref) = &filter.extension_ref {
                        // Check if this is a reference to our StaticResponseFilter
                        if extension_ref.group == "vale-gateway.whitefamily.in"
                            && extension_ref.kind == "StaticResponseFilter"
                            && extension_ref.name == filter_name
                        {
                            // For extension refs, we assume same namespace as the route since
                            // the HTTPRoute extension ref doesn't have a namespace field
                            let route_namespace = route.metadata.namespace.as_deref();
                            if route_namespace == Some(filter_namespace) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Create status for a `StaticResponseFilter` based on its spec and attachment info
fn create_filter_status(
    spec: &vg_api::v1alpha1::StaticResponseFilterSpec,
    attached_routes: i32,
) -> StaticResponseFilterStatus {
    let now = Time(Utc::now());
    let mut conditions = Vec::new();

    // Accepted condition - validate the filter configuration
    let accepted_condition =
        if is_valid_status_code(spec.status_code) && is_valid_body_config(spec.body.as_ref()) {
            Condition {
                type_: StaticResponseFilterConditionType::Accepted
                    .as_str()
                    .to_string(),
                status: "True".to_string(),
                reason: StaticResponseFilterConditionReason::Accepted
                    .as_str()
                    .to_string(),
                message: "StaticResponseFilter configuration is valid".to_string(),
                last_transition_time: now.clone(),
                observed_generation: None,
            }
        } else {
            Condition {
                type_: StaticResponseFilterConditionType::Accepted
                    .as_str()
                    .to_string(),
                status: "False".to_string(),
                reason: StaticResponseFilterConditionReason::InvalidConfiguration
                    .as_str()
                    .to_string(),
                message: "StaticResponseFilter configuration is invalid".to_string(),
                last_transition_time: now.clone(),
                observed_generation: None,
            }
        };
    conditions.push(accepted_condition);

    // Ready condition - filter is ready if it's accepted
    let ready_condition = if conditions[0].status == "True" {
        Condition {
            type_: StaticResponseFilterConditionType::Ready
                .as_str()
                .to_string(),
            status: "True".to_string(),
            reason: StaticResponseFilterConditionReason::Ready
                .as_str()
                .to_string(),
            message: "StaticResponseFilter is ready to serve responses".to_string(),
            last_transition_time: now.clone(),
            observed_generation: None,
        }
    } else {
        Condition {
            type_: StaticResponseFilterConditionType::Ready
                .as_str()
                .to_string(),
            status: "False".to_string(),
            reason: StaticResponseFilterConditionReason::NotReady
                .as_str()
                .to_string(),
            message: "StaticResponseFilter is not ready due to invalid configuration".to_string(),
            last_transition_time: now.clone(),
            observed_generation: None,
        }
    };
    conditions.push(ready_condition);

    // Attached condition - whether the filter is attached to any routes
    let attached_condition = if attached_routes > 0 {
        Condition {
            type_: StaticResponseFilterConditionType::Attached
                .as_str()
                .to_string(),
            status: "True".to_string(),
            reason: StaticResponseFilterConditionReason::AttachedToRoute
                .as_str()
                .to_string(),
            message: format!("StaticResponseFilter is attached to {attached_routes} route(s)"),
            last_transition_time: now.clone(),
            observed_generation: None,
        }
    } else {
        Condition {
            type_: StaticResponseFilterConditionType::Attached
                .as_str()
                .to_string(),
            status: "False".to_string(),
            reason: StaticResponseFilterConditionReason::NotAttached
                .as_str()
                .to_string(),
            message: "StaticResponseFilter is not attached to any routes".to_string(),
            last_transition_time: now.clone(),
            observed_generation: None,
        }
    };
    conditions.push(attached_condition);

    StaticResponseFilterStatus {
        conditions: Some(conditions),
        attached_routes,
        last_updated: Some(now),
    }
}

/// Validate if the status code is valid HTTP status code
fn is_valid_status_code(status_code: u16) -> bool {
    // Valid HTTP status codes are typically 100-599
    (100..=599).contains(&status_code)
}

/// Validate if the body configuration is valid
fn is_valid_body_config(body: Option<&vg_api::v1alpha1::StaticResponseFilterBody>) -> bool {
    if let Some(body_config) = body {
        match body_config.format {
            vg_api::v1alpha1::StaticResponseFilterBodyFormat::Text => body_config.text.is_some(),
            vg_api::v1alpha1::StaticResponseFilterBodyFormat::Binary => {
                body_config.binary.is_some()
            }
        }
    } else {
        // No body is valid (e.g., for status-only responses)
        true
    }
}

/// Update the status of a `StaticResponseFilter`
async fn update_filter_status(
    client: Client,
    filter: &StaticResponseFilter,
    status: StaticResponseFilterStatus,
) -> Result<()> {
    let filter_name = filter
        .metadata
        .name
        .as_ref()
        .context("Filter name not found")?;
    let filter_namespace = filter
        .metadata
        .namespace
        .as_ref()
        .context("Filter namespace not found")?;

    let api: Api<StaticResponseFilter> = Api::namespaced(client, filter_namespace);

    debug!(
        "Updating status for StaticResponseFilter {}/{}",
        filter_namespace, filter_name
    );

    // Retry mechanism to handle conflicts (optimistic concurrency control)
    let max_retries = 5;
    let mut attempt = 0;

    while attempt < max_retries {
        attempt += 1;

        // Get the latest version of the filter
        let current_filter = api
            .get_status(filter_name)
            .instrument(info_span!("get_static_response_filter_status"))
            .await
            .with_context(|| {
                format!(
                    "Failed to get current status of StaticResponseFilter {filter_namespace}/{filter_name}"
                )
            })?;

        // Check if the status actually needs to be updated
        if let Some(existing_status) = &current_filter.status
            && existing_status == &status {
                debug!(
                    "Status for StaticResponseFilter {}/{} is already up to date",
                    filter_namespace, filter_name
                );
                return Ok(());
            }

        // Create a new version with updated status
        let mut updated_filter = current_filter.clone();
        updated_filter.status = Some(status.clone());

        // Attempt to update the status
        match api
            .replace_status(
                filter_name,
                &PostParams::default(),
                serde_json::to_vec(&updated_filter)?,
            )
            .instrument(info_span!("replace_static_response_filter_status"))
            .await
        {
            Ok(_) => {
                debug!(
                    "Successfully updated status for StaticResponseFilter {}/{} on attempt {}",
                    filter_namespace, filter_name, attempt
                );
                return Ok(());
            }
            Err(kube::Error::Api(api_error)) if api_error.code == 409 => {
                // Conflict error - resource was modified, retry
                warn!(
                    "Conflict updating StaticResponseFilter {}/{} status on attempt {}, retrying...",
                    filter_namespace, filter_name, attempt
                );
                if attempt >= max_retries {
                    return Err(anyhow::anyhow!(
                        "Failed to update status after {} attempts due to conflicts: {}",
                        max_retries,
                        api_error
                    ));
                }
                // Brief delay before retry to avoid tight retry loops
                tokio::time::sleep(tokio::time::Duration::from_millis(100 * attempt as u64)).await;
                continue;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to update status of StaticResponseFilter {}/{}: {}",
                    filter_namespace,
                    filter_name,
                    e
                ));
            }
        }
    }

    Err(anyhow::anyhow!(
        "Exhausted all {} retry attempts for StaticResponseFilter {}/{}",
        max_retries,
        filter_namespace,
        filter_name
    ))
}
