use kube::Client;
use kube_leader_election::{LeaseLock, LeaseLockParams};
use kubera_core::continue_after;
use kubera_core::sync::signal::{channel, Receiver};
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tracing::debug;
use tracing::log::warn;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ControlplaneRole {
    Undetermined,
    Primary,
    Redundant,
}

pub fn watch_role(
    join_set: &mut JoinSet<()>,
    client: &Client,
    namespace: &str,
    instance_name: &str,
    pod_name: &str,
) -> Receiver<ControlplaneRole> {
    let (tx, rx) = channel(ControlplaneRole::Undetermined);

    let lock = LeaseLock::new(
        client.clone(),
        namespace,
        LeaseLockParams {
            holder_id: pod_name.to_string(),
            lease_name: format!("{}-primary-lock", instance_name),
            lease_ttl: Duration::from_secs(30),
        },
    );

    join_set.spawn(async move {
        loop {
            let role = match lock.try_acquire_or_renew().await {
                Ok(lease) if lease.acquired_lease => {
                    debug!("Acquired lease, assuming primary role");
                    ControlplaneRole::Primary
                }
                Ok(_) => {
                    debug!("Lease renewed, assuming redundant role");
                    ControlplaneRole::Redundant
                }
                Err(e) => {
                    warn!("Failed to acquire or renew lease: {}", e);
                    ControlplaneRole::Undetermined
                }
            };

            tx.replace(role);

            continue_after!(Duration::from_secs(10));
        }

        if tx.current().as_ref() == &ControlplaneRole::Primary {
            let _ = lock.step_down();
        }
    });

    rx
}
