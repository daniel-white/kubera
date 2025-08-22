use crate::kubernetes::objects::{ObjectRef, Objects};
use gateway_api::httproutes::{HTTPRoute, HTTPRouteRulesFiltersType};
use getset::Getters;
use std::collections::HashMap;
use std::sync::Arc;
use strum::{EnumString, IntoStaticStr};
use tracing::{debug, info};
use vg_api::v1alpha1::{AccessControlFilter, StaticResponseFilter};
use vg_core::continue_on;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, ReadyState};

#[derive(Debug, Clone, PartialEq, Eq, EnumString, IntoStaticStr)]
pub enum ExtensionFilterKind {
    StaticResponseFilter,
    AccessControlFilter,
}

#[derive(Debug, Default, Clone, PartialEq, Getters)]
pub struct ExtensionFilters {
    #[getset(get = "pub")]
    static_responses: Objects<StaticResponseFilter>,
    #[getset(get = "pub")]
    access_controls: Objects<AccessControlFilter>,
}

pub fn collect_extension_filters_by_gateway(
    task_builder: &TaskBuilder,
    http_routes_by_gateway_rx: &Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    static_response_filters_rx: &Receiver<Objects<StaticResponseFilter>>,
    access_control_filters_rx: &Receiver<Objects<AccessControlFilter>>,
) -> Receiver<HashMap<ObjectRef, ExtensionFilters>> {
    let (tx, rx) = signal("collected_extension_filters_by_gateway");

    let http_routes_by_gateway_rx = http_routes_by_gateway_rx.clone();
    let static_response_filters_rx = static_response_filters_rx.clone();
    let access_control_filters_rx = access_control_filters_rx.clone();

    task_builder
        .new_task(stringify!(pub fn collect_extension_filters_by_gateway))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((
                    http_routes_by_gateway,
                    static_response_filters,
                    access_control_filters,
                )) = await_ready!(
                    http_routes_by_gateway_rx,
                    static_response_filters_rx,
                    access_control_filters_rx
                ) {
                    let mut filters: HashMap<ObjectRef, ExtensionFilters> = HashMap::new();

                    for (gateway_ref, http_routes) in http_routes_by_gateway {
                        info!(
                            "Collecting ExtensionRef filters for Gateway: object.ref={}",
                            gateway_ref
                        );

                        let extension_filters = filters.entry(gateway_ref.clone()).or_default();

                        for filter in http_routes
                            .iter()
                            .flat_map(|r| r.spec.rules.iter().flatten())
                            .flat_map(|r| r.filters.iter().flatten())
                            .filter_map(|f| {
                                if f.r#type == HTTPRouteRulesFiltersType::ExtensionRef {
                                    f.extension_ref.as_ref()
                                } else {
                                    None
                                }
                            })
                            .filter(|f| &f.group == "vale-gateway.whitefamily.in")
                        {
                            let kind = ExtensionFilterKind::try_from(filter.kind.as_str());
                            if Ok(ExtensionFilterKind::StaticResponseFilter) == kind {
                                let filter_ref = ObjectRef::of_kind::<StaticResponseFilter>()
                                    .namespace(gateway_ref.namespace().clone())
                                    .name(&filter.name)
                                    .build();

                                if let Some(static_response_filter) =
                                    static_response_filters.get_by_ref(&filter_ref)
                                {
                                    let _ = extension_filters
                                        .static_responses
                                        .insert(static_response_filter);
                                }
                            } else if Ok(ExtensionFilterKind::AccessControlFilter) == kind {
                                let filter_ref = ObjectRef::of_kind::<AccessControlFilter>()
                                    .namespace(gateway_ref.namespace().clone())
                                    .name(&filter.name)
                                    .build();

                                if let Some(access_control_filter) =
                                    access_control_filters.get_by_ref(&filter_ref)
                                {
                                    let _ = extension_filters
                                        .access_controls
                                        .insert(access_control_filter);
                                }
                            }
                        }
                    }

                    tx.set(filters).await;
                }

                continue_on!(
                    http_routes_by_gateway_rx.changed(),
                    static_response_filters_rx.changed(),
                    access_control_filters_rx.changed()
                );
            }
        });

    rx
}
