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
use kubera_core::sync::signal::{Receiver, Sender, signal};
use kubera_core::task::Builder as TaskBuilder;
use kubera_core::{continue_after, continue_on};
use std::future::ready;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::select;
use tracing::log::warn;
use tracing::{debug, info, instrument};

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
    let (tx, rx) = signal();
    let kube_client_rx = kube_client_rx.clone();
    let instance_role_rx = instance_role_rx.clone();

    task_builder
        .new_task(stringify!(watch_leader_instance_ip_addr))
        .spawn(async move {
        let controller_context = Arc::new(ControllerContext { options, tx });
        loop {
            match (kube_client_rx.get().await.as_deref(), instance_role_rx.get().await.as_ref()) {
                (Some(kube_client), Some(instance_role)) => {
                    let primary_pod_ref = instance_role.primary_pod_ref();
                    info!("Determining instance IP address for pod as it is the primary: {}", primary_pod_ref);
                    let api = Api::<Pod>::namespaced(
                        kube_client.clone().into(),
                        primary_pod_ref
                            .namespace()
                            .as_deref()
                            .expect("Namespace should be present"),
                    );
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
}

impl InstanceRole {
    pub fn primary_pod_ref(&self) -> ObjectRef {
        match self {
            InstanceRole::Primary(pod_ref) | InstanceRole::Redundant(pod_ref) => pod_ref.clone(),
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
    let (tx, rx) = signal();

    let namespace = namespace.to_string();
    let instance_name = instance_name.to_string();
    let pod_name = pod_name.to_string();

    task_builder
        .new_task(stringify!(determine_instance_role))
        .spawn(async move {
            let kube_client = Client::try_default()
                .await
                .expect("Unable to start client to determine role");
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
                        debug!("Acquired lease, assuming primary role");
                        let pod_ref = get_pod_ref(&namespace, &lease);
                        tx.set(InstanceRole::Primary(pod_ref)).await;
                    }
                    Ok(LeaseLockResult::NotAcquired(lease)) => {
                        debug!("Lease renewed, assuming redundant role");
                        let pod_ref = get_pod_ref(&namespace, &lease);
                        tx.set(InstanceRole::Redundant(pod_ref)).await;
                    }
                    Err(err) => warn!("Failed to acquire or renew lease: {err}"),
                };

                continue_after!(options.lease_check_interval());
            }
        });

    rx
}

fn get_pod_ref(namespace: &str, lease: &Lease) -> ObjectRef {
    let lease = lease
        .spec
        .as_ref()
        .expect("Lease spec should be present when determining pod reference");
    let holder_id = lease
        .holder_identity
        .clone()
        .expect("Holder identity should be present in lease spec");

    ObjectRef::new_builder()
        .of_kind::<Pod>()
        .namespace(Some(namespace.to_string()))
        .name(holder_id)
        .build()
        .expect("Failed to build ObjectRef for Pod")
}
