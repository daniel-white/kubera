use crate::kubernetes::objects::ObjectRef;
use crate::kubernetes::KubeClientCell;
use futures::StreamExt;
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::runtime::Controller;
use kube::{Api, Client};
use kube_leader_election::{LeaseLock, LeaseLockParams, LeaseLockResult};
use kubera_core::sync::signal::{channel, Receiver, Sender};
use kubera_core::{continue_after, continue_on};
use std::future::ready;
use std::net::IpAddr;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::select;
use tokio::task::JoinSet;
use tracing::log::warn;
use tracing::{debug, instrument};

pub fn watch_leader_instance_ip_addr(
    join_set: &mut JoinSet<()>,
    kube_client: &Receiver<Option<KubeClientCell>>,
    instance_role: &Receiver<InstanceRole>,
) -> Receiver<Option<IpAddr>> {
    struct ControllerContext {
        tx: Sender<Option<IpAddr>>,
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

        warn!(
            "Reconcile called for pod in namespace with IP {:?}",
            ip_addr
        );
        ctx.tx.replace(ip_addr);
        Ok(Action::requeue(Duration::from_secs(60)))
    }

    fn error_policy(_: Arc<Pod>, _: &ControllerError, _: Arc<ControllerContext>) -> Action {
        Action::requeue(Duration::from_secs(5))
    }

    let instance_role = instance_role.clone();
    let kube_client = kube_client.clone();
    let (tx, rx) = channel(None);

    join_set.spawn(async move {
        let controller_context = Arc::new(ControllerContext { tx });
        loop {
            if let Some(kube_client2) = kube_client.current().as_ref()
                && let Some(primary_pod_ref) = instance_role.current().primary_pod_ref()
            {
                let api = Api::<Pod>::namespaced(
                    kube_client2.deref().clone(),
                    primary_pod_ref.namespace().clone().unwrap().as_str(),
                );
                let config = Config::default()
                    .fields(format!("metadata.name={}", primary_pod_ref.name()).as_str());
                let controller = Controller::new(api, config)
                    .shutdown_on_signal()
                    .run(reconcile, error_policy, controller_context.clone())
                    .filter_map(|x| async move { Some(x) })
                    .for_each(|_| ready(()));
                select! {
                    _ = controller => {
                        break;
                    },
                    _ = kube_client.changed() => {
                        continue;
                    }
                    _ = instance_role.changed() => {
                        continue;
                    }
                };
            } else {
                controller_context.tx.replace(None);
                continue_on!(kube_client.changed(), instance_role.changed());
            }
        }
    });

    rx
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum InstanceRole {
    Undetermined,
    Primary(ObjectRef),
    Redundant(ObjectRef),
}

impl InstanceRole {
    pub fn primary_pod_ref(&self) -> Option<&ObjectRef> {
        match self {
            InstanceRole::Primary(pod_ref) => Some(pod_ref),
            InstanceRole::Redundant(pod_ref) => Some(pod_ref),
            InstanceRole::Undetermined => None,
        }
    }

    pub fn is_primary(&self) -> bool {
        matches!(self, InstanceRole::Primary(_))
    }
}

pub fn determine_instance_role(
    join_set: &mut JoinSet<()>,
    namespace: &str,
    instance_name: &str,
    pod_name: &str,
) -> Receiver<InstanceRole> {
    let (tx, rx) = channel(InstanceRole::Undetermined);

    let namespace = namespace.to_string();
    let instance_name = instance_name.to_string();
    let pod_name = pod_name.to_string();

    join_set.spawn(async move {
        let kube_client = Client::try_default()
            .await
            .expect("Unable to start client to determine role");
        let lock = LeaseLock::new(
            kube_client,
            &namespace,
            LeaseLockParams {
                holder_id: pod_name.to_string(),
                lease_name: format!("{instance_name}-primary"),
                lease_ttl: Duration::from_secs(30),
            },
        );

        loop {
            let new_role = match lock.try_acquire_or_renew().await {
                Ok(LeaseLockResult::Acquired(lease)) => {
                    debug!("Acquired lease, assuming primary role");
                    let pod_ref = get_pod_ref(&namespace, lease);
                    Some(InstanceRole::Primary(pod_ref))
                }
                Ok(LeaseLockResult::NotAcquired(lease)) => {
                    debug!("Lease renewed, assuming redundant role");
                    let pod_ref = get_pod_ref(&namespace, lease);
                    Some(InstanceRole::Redundant(pod_ref))
                }
                Err(e) => {
                    warn!("Failed to acquire or renew lease: {e}");
                    None
                }
            };

            if let Some(new_role) = new_role {
                tx.replace(new_role);
            }

            continue_after!(Duration::from_secs(10));
        }
    });

    rx
}

fn get_pod_ref(namespace: &str, lease: Lease) -> ObjectRef {
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
