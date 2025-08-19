use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::GatewayInstanceConfiguration;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::apps::v1::Deployment;
use kube::runtime::watcher::Config;
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::{
    GatewayInstrumentationOpenTelemetry, GatewayInstrumentationOpenTelemetryParentBasedType,
    GatewayInstrumentationOpenTelemetrySamplingType,
};
use vg_core::continue_after;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

const TEMPLATE: &str = include_str!("./templates/gateway_deployment.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct OpenTelemetryTemplateValues {
    #[builder(setter(into))]
    collector_name: String,
    #[builder(setter(into))]
    exporter_endpoint: String,
    #[builder(setter(into))]
    traces_sampler: String,
    #[builder(setter(into))]
    traces_sampler_arg: String,
}

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into))]
    cluster_name: String,
    #[builder(setter(into))]
    configmap_name: String,
    #[builder(setter(into))]
    image_pull_policy: String,
    #[builder(setter(into))]
    image_repository: String,
    #[builder(setter(into))]
    image_tag: String,
    replicas: i32,
    open_telemetry: OpenTelemetryTemplateValues,
}

pub fn sync_gateway_deployments(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
    gateway_instances_rx: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let (tx, current_service_refs_rx) = sync_objects!(
        options,
        task_builder,
        Deployment,
        kube_client_rx,
        instance_role_rx,
        TemplateValues,
        TEMPLATE
    );
    generate_gateway_deployments(
        options,
        task_builder,
        tx,
        current_service_refs_rx,
        gateway_instances_rx,
    );
}

fn extract_traces_sampler_config(
    open_telemetry: Option<&GatewayInstrumentationOpenTelemetry>,
) -> (Option<String>, Option<String>) {
    if let Some(otel) = open_telemetry {
        if let Some(sampling) = otel.sampling.as_ref() {
            let sampler = match sampling.sampling_type {
                Some(GatewayInstrumentationOpenTelemetrySamplingType::TraceIdRatioBased) => {
                    Some("traceidratio".to_string())
                }
                Some(GatewayInstrumentationOpenTelemetrySamplingType::ParentBased) => {
                    if let Some(pb) = sampling.parent_based.as_ref() {
                        if pb.parent_type == Some(GatewayInstrumentationOpenTelemetryParentBasedType::TraceIdRatioBased) {
                            Some("parentbased_traceidratio".to_string())
                        } else if pb.parent_type == Some(GatewayInstrumentationOpenTelemetryParentBasedType::AlwaysOff) {
                            Some("parentbased_always_off".to_string())
                        } else {
                            Some("parentbased_always_on".to_string())
                        }
                    } else {
                        Some("parentbased_always_on".to_string())
                    }
                }
                Some(GatewayInstrumentationOpenTelemetrySamplingType::AlwaysOn) => {
                    Some("always_on".to_string())
                }
                Some(GatewayInstrumentationOpenTelemetrySamplingType::AlwaysOff) => {
                    Some("always_off".to_string())
                }
                _ => None,
            };
            let arg = match sampling.sampling_type {
                Some(GatewayInstrumentationOpenTelemetrySamplingType::TraceIdRatioBased) => {
                    sampling
                        .trace_id_ratio_based
                        .as_ref()
                        .and_then(|r| r.ratio)
                        .map(|r| r.to_string())
                }
                Some(GatewayInstrumentationOpenTelemetrySamplingType::ParentBased) => {
                    if let Some(pb) = sampling.parent_based.as_ref() {
                        if pb.parent_type == Some(GatewayInstrumentationOpenTelemetryParentBasedType::TraceIdRatioBased) {
                            pb.trace_id_ratio_based.as_ref().and_then(|r| r.ratio).map(|r| r.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            (sampler, arg)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    }
}

fn generate_gateway_deployments(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    tx: UnboundedSender<SyncObjectAction<TemplateValues, Deployment>>,
    current_service_refs_rx: Receiver<HashSet<ObjectRef>>,
    gateway_instances_rx: &Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
) {
    let gateway_instances_rx = gateway_instances_rx.clone();

    task_builder
        .new_task(stringify!(generate_gateway_deployments))
        .spawn(async move {
            loop {
                await_ready!(gateway_instances_rx, current_service_refs_rx)
                    .and_then(async |gateway_instances, current_service_refs| {
                        let desired_deployments: Vec<_> = gateway_instances
                            .iter()
                            .map(|(gateway_ref, instance)| {
                                let deployment_ref = ObjectRef::of_kind::<Deployment>()
                                    .namespace(gateway_ref.namespace().clone())
                                    .name(gateway_ref.name())
                                    .build();

                                let replicas = instance
                                    .deployment_overrides()
                                    .spec
                                    .as_ref()
                                    .and_then(|spec| spec.replicas)
                                    .unwrap_or(1);

                                let (traces_sampler, traces_sampler_arg) =
                                    extract_traces_sampler_config(
                                        instance.merged_open_telemetry().as_ref(),
                                    );
                                let open_telemetry = OpenTelemetryTemplateValues::builder()
                                    .collector_name(
                                        env::var("OTEL_COLLECTOR_NAME").unwrap_or_default(),
                                    )
                                    .exporter_endpoint(
                                        env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_default(),
                                    )
                                    .traces_sampler(traces_sampler.clone().unwrap_or_default())
                                    .traces_sampler_arg(
                                        traces_sampler_arg.clone().unwrap_or_default(),
                                    )
                                    .build();
                                let template_values = TemplateValues::builder()
                                    .gateway_name(gateway_ref.name())
                                    .cluster_name("TBD")
                                    .configmap_name(format!("{}-config", gateway_ref.name()))
                                    .image_pull_policy(Into::<&'static str>::into(
                                        instance.image_pull_policy(),
                                    ))
                                    .image_repository(instance.image_repository().to_string())
                                    .image_tag(instance.image_tag().to_string())
                                    .replicas(replicas)
                                    .open_telemetry(open_telemetry)
                                    .build();

                                (
                                    deployment_ref,
                                    gateway_ref,
                                    template_values,
                                    instance.deployment_overrides(),
                                )
                            })
                            .collect();

                        let desired_deployments_ref: HashSet<_> = desired_deployments
                            .iter()
                            .map(|(ref_, _, _, _)| ref_.clone())
                            .collect();

                        let deleted_refs =
                            current_service_refs.difference(&desired_deployments_ref);
                        for deleted_ref in deleted_refs {
                            let _ = tx
                                .send(SyncObjectAction::Delete(deleted_ref.clone()))
                                .inspect_err(|err| {
                                    warn!(
                                        "Failed to send delete action for deployment {}: {}",
                                        deleted_ref, err
                                    );
                                });
                        }

                        for (deployment_ref, gateway_ref, template_values, deployment_overrides) in
                            desired_deployments
                        {
                            tx.send(SyncObjectAction::Upsert(
                                deployment_ref.clone(),
                                gateway_ref.clone(),
                                template_values,
                                Some(deployment_overrides.clone()),
                            ))
                            .inspect_err(|err| {
                                warn!(
                                    "Failed to send upsert action for deployment {}: {}",
                                    deployment_ref, err
                                );
                            })
                            .ok();
                        }
                    })
                    .run()
                    .await;

                continue_after!(
                    options.auto_cycle_duration(),
                    gateway_instances_rx.changed(),
                    current_service_refs_rx.changed()
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use vg_api::v1alpha1::{
        GatewayInstrumentationOpenTelemetry, GatewayInstrumentationOpenTelemetryParentBased,
        GatewayInstrumentationOpenTelemetryParentBasedType,
        GatewayInstrumentationOpenTelemetrySampling,
        GatewayInstrumentationOpenTelemetrySamplingType,
        GatewayInstrumentationOpenTelemetryTraceIdRatioBased,
    };

    #[test]
    fn test_extract_traces_sampler_config_table() {
        struct Case {
            name: &'static str,
            input: Option<GatewayInstrumentationOpenTelemetry>,
            expected: (Option<String>, Option<String>),
        }
        let cases = vec![
            Case {
                name: "TraceIdRatioBased with ratio",
                input: Some(GatewayInstrumentationOpenTelemetry {
                    collector: None,
                    exporter: None,
                    sampling: Some(GatewayInstrumentationOpenTelemetrySampling {
                        sampling_type: Some(GatewayInstrumentationOpenTelemetrySamplingType::TraceIdRatioBased),
                        trace_id_ratio_based: Some(GatewayInstrumentationOpenTelemetryTraceIdRatioBased { ratio: Some(0.42) }),
                        parent_based: None,
                    }),
                }),
                expected: (Some("traceidratio".to_string()), Some("0.42".to_string())),
            },
            Case {
                name: "ParentBased TraceIdRatioBased with ratio",
                input: Some(GatewayInstrumentationOpenTelemetry {
                    collector: None,
                    exporter: None,
                    sampling: Some(GatewayInstrumentationOpenTelemetrySampling {
                        sampling_type: Some(GatewayInstrumentationOpenTelemetrySamplingType::ParentBased),
                        trace_id_ratio_based: None,
                        parent_based: Some(GatewayInstrumentationOpenTelemetryParentBased {
                            parent_type: Some(GatewayInstrumentationOpenTelemetryParentBasedType::TraceIdRatioBased),
                            trace_id_ratio_based: Some(GatewayInstrumentationOpenTelemetryTraceIdRatioBased { ratio: Some(0.99) }),
                        }),
                    }),
                }),
                expected: (Some("parentbased_traceidratio".to_string()), Some("0.99".to_string())),
            },
            Case {
                name: "ParentBased AlwaysOff",
                input: Some(GatewayInstrumentationOpenTelemetry {
                    collector: None,
                    exporter: None,
                    sampling: Some(GatewayInstrumentationOpenTelemetrySampling {
                        sampling_type: Some(GatewayInstrumentationOpenTelemetrySamplingType::ParentBased),
                        trace_id_ratio_based: None,
                        parent_based: Some(GatewayInstrumentationOpenTelemetryParentBased {
                            parent_type: Some(GatewayInstrumentationOpenTelemetryParentBasedType::AlwaysOff),
                            trace_id_ratio_based: None,
                        }),
                    }),
                }),
                expected: (Some("parentbased_always_off".to_string()), None),
            },
            Case {
                name: "ParentBased AlwaysOn (default)",
                input: Some(GatewayInstrumentationOpenTelemetry {
                    collector: None,
                    exporter: None,
                    sampling: Some(GatewayInstrumentationOpenTelemetrySampling {
                        sampling_type: Some(GatewayInstrumentationOpenTelemetrySamplingType::ParentBased),
                        trace_id_ratio_based: None,
                        parent_based: Some(GatewayInstrumentationOpenTelemetryParentBased {
                            parent_type: None,
                            trace_id_ratio_based: None,
                        }),
                    }),
                }),
                expected: (Some("parentbased_always_on".to_string()), None),
            },
            Case {
                name: "AlwaysOn",
                input: Some(GatewayInstrumentationOpenTelemetry {
                    collector: None,
                    exporter: None,
                    sampling: Some(GatewayInstrumentationOpenTelemetrySampling {
                        sampling_type: Some(GatewayInstrumentationOpenTelemetrySamplingType::AlwaysOn),
                        trace_id_ratio_based: None,
                        parent_based: None,
                    }),
                }),
                expected: (Some("always_on".to_string()), None),
            },
            Case {
                name: "AlwaysOff",
                input: Some(GatewayInstrumentationOpenTelemetry {
                    collector: None,
                    exporter: None,
                    sampling: Some(GatewayInstrumentationOpenTelemetrySampling {
                        sampling_type: Some(GatewayInstrumentationOpenTelemetrySamplingType::AlwaysOff),
                        trace_id_ratio_based: None,
                        parent_based: None,
                    }),
                }),
                expected: (Some("always_off".to_string()), None),
            },
            Case {
                name: "None config",
                input: None,
                expected: (None, None),
            },
            Case {
                name: "No sampling field",
                input: Some(GatewayInstrumentationOpenTelemetry { collector: None, exporter: None, sampling: None }),
                expected: (None, None),
            },
        ];
        for case in cases {
            let got = super::extract_traces_sampler_config(case.input.as_ref());
            assert_eq!(got, case.expected, "case: {}", case.name);
        }
    }
}
