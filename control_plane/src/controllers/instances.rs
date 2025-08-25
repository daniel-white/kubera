use crate::instrumentation::METER;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::ObjectRef;
use crate::options::Options;
use futures::StreamExt;
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::Controller;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::{Api, Client};
use kube_leader_election::{LeaseLock, LeaseLockParams, LeaseLockResult};
use opentelemetry::KeyValue;
use std::future::ready;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::select;
use tracing::log::warn;
use tracing::{debug, info, instrument};
use vg_core::sync::signal::{Receiver, Sender, signal};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{ReadyState, await_ready, continue_after, continue_on};

pub fn watch_leader_instance_ip_addr(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    kube_client_rx: &Receiver<KubeClientCell>,
    instance_role_rx: &Receiver<InstanceRole>,
) -> Receiver<IpAddr> {
    struct ControllerContext {
        options: Arc<Options>,
        tx: Sender<IpAddr>,
    }

    #[derive(Error, Debug)]
    enum ControllerError {}

    #[instrument(skip(pod, ctx))]
    async fn reconcile(
        pod: Arc<Pod>,
        ctx: Arc<ControllerContext>,
    ) -> Result<Action, ControllerError> {
        let ip_addr = pod
            .status
            .as_ref()
            .and_then(|s| s.pod_ip.as_ref())
            .and_then(|ip| IpAddr::from_str(ip.as_str()).ok());

        ctx.tx.replace(ip_addr).await;
        Ok(Action::requeue(ctx.options.controller_requeue_duration()))
    }

    #[allow(clippy::needless_pass_by_value)]
    fn error_policy(_: Arc<Pod>, _: &ControllerError, ctx: Arc<ControllerContext>) -> Action {
        Action::requeue(ctx.options.controller_error_requeue_duration())
    }
    let (tx, rx) = signal("leader_instance_ip_addr");
    let kube_client_rx = kube_client_rx.clone();
    let instance_role_rx = instance_role_rx.clone();

    task_builder
        .new_task(stringify!(watch_leader_instance_ip_addr))
        .spawn(async move {
            let controller_context = Arc::new(ControllerContext { options, tx });
            loop {
                match (kube_client_rx.get().await.as_deref(), instance_role_rx.get().await.as_ref().and_then(|r| r.primary_pod_ref().cloned())) {
                    (Some(kube_client), Some(primary_pod_ref)) => {
                        info!("Determining instance IP address for pod as it is the primary: {:?}", primary_pod_ref);

                        if let Some(namespace) = primary_pod_ref.namespace().as_deref() {
                            let api = Api::<Pod>::namespaced(kube_client.clone(), namespace);
                            let config = Config::default()
                                .fields(format!("metadata.name={}", primary_pod_ref.name()).as_str());
                            let controller = Controller::new(api, config)
                                .shutdown_on_signal()
                                .run(reconcile, error_policy, controller_context.clone())
                                .for_each(|_| ready(()));
                            select! {
                            () = controller => {
                                break;
                            },
                            _ = kube_client_rx.changed() => {
                                continue;
                            }
                            _ = instance_role_rx.changed() => {
                                continue;
                            }
                        }
                        } else {
                            warn!("Primary pod reference missing namespace, cannot determine instance IP");
                        }
                    }
                    (None, _) => {
                        debug!("Kube client is not available, unable to determine primary instance IP address");
                    }
                    (_, None) => {
                        debug!("Instance role is not available, unable to determine primary instance IP address");
                    }
                }

                continue_on!(kube_client_rx.changed(), instance_role_rx.changed());
            }
        });

    rx
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum InstanceRole {
    Primary(ObjectRef),
    Redundant(ObjectRef),
    Undetermined,
}

impl InstanceRole {
    pub fn primary_pod_ref(&self) -> Option<&ObjectRef> {
        match self {
            InstanceRole::Primary(pod_ref) | InstanceRole::Redundant(pod_ref) => Some(pod_ref),
            InstanceRole::Undetermined => None,
        }
    }

    pub fn is_primary(&self) -> bool {
        matches!(self, InstanceRole::Primary(_))
    }
}

pub fn determine_instance_role(
    options: Arc<Options>,
    task_builder: &TaskBuilder,
    namespace: &str,
    instance_name: &str,
    pod_name: &str,
) -> Receiver<InstanceRole> {
    let (tx, rx) = signal("instance_role");

    let namespace = namespace.to_string();
    let instance_name = instance_name.to_string();
    let pod_name = pod_name.to_string();

    task_builder
        .new_task(stringify!(determine_instance_role))
        .spawn(async move {
            match Client::try_default().await {
                Ok(kube_client) => {
                    let lock = LeaseLock::new(
                        kube_client,
                        &namespace,
                        LeaseLockParams {
                            holder_id: pod_name.to_string(),
                            lease_name: format!("{instance_name}-primary"),
                            lease_ttl: options.lease_duration(),
                        },
                    );

                    loop {
                        match lock.try_acquire_or_renew().await {
                            Ok(LeaseLockResult::Acquired(lease)) => {
                                if let Some(pod_ref) = get_pod_ref(&namespace, &lease) {
                                    debug!("Acquired lease, assuming primary role");
                                    tx.set(InstanceRole::Primary(pod_ref)).await;
                                } else {
                                    warn!("Lease acquired and pod reference is missing, assuming undetermined role");
                                    tx.set(InstanceRole::Undetermined).await;
                                }
                            }
                            Ok(LeaseLockResult::NotAcquired(lease)) => {
                                if let Some(pod_ref) = get_pod_ref(&namespace, &lease) {
                                    debug!("Lease not acquired, assuming redundant role");
                                    tx.set(InstanceRole::Redundant(pod_ref)).await;
                                } else {
                                    warn!("Lease not acquired and pod reference is missing, assuming undetermined role");
                                    tx.set(InstanceRole::Undetermined).await;
                                }
                            }
                            Err(err) => {
                                warn!("Failed to acquire or renew lease: {err}");
                                tx.set(InstanceRole::Undetermined).await;
                            }
                        }

                        continue_after!(options.lease_check_interval());
                    }
                }
                Err(err) => {
                    warn!("Unable to start Kubernetes client to determine role: {err}");
                    tx.set(InstanceRole::Undetermined).await;
                }
            }
        });

    report_instance_role(task_builder, rx.clone());

    rx
}

fn get_pod_ref(namespace: &str, lease: &Lease) -> Option<ObjectRef> {
    let lease = lease.spec.as_ref()?;
    let holder_id = lease.holder_identity.as_ref()?;

    let object_ref = ObjectRef::of_kind::<Pod>()
        .namespace(Some(namespace.to_string()))
        .name(holder_id)
        .build();

    Some(object_ref)
}

fn report_instance_role(task_builder: &TaskBuilder, instance_role_rx: Receiver<InstanceRole>) {
    task_builder
        .new_task(stringify!(report_instance_role))
        .spawn(async move {
            let metric = METER
                .u64_gauge("vg_control_plane_instance_role")
                .with_description("Indicates the current state of the instance")
                .build();
            loop {
                if let ReadyState::Ready(instance_role) = await_ready!(instance_role_rx) {
                    let (primary_value, redundant_value, undetermined_value) = match instance_role {
                        InstanceRole::Primary(_) => (1, 0, 0),
                        InstanceRole::Redundant(_) => (0, 1, 0),
                        InstanceRole::Undetermined => (0, 0, 1),
                    };

                    metric.record(primary_value, &[KeyValue::new("role", "primary")]);
                    metric.record(redundant_value, &[KeyValue::new("role", "redundant")]);
                    metric.record(undetermined_value, &[KeyValue::new("role", "undetermined")]);
                }
                continue_after!(Duration::from_secs(10), instance_role_rx.changed());
            }
        });
}
