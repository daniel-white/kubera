use futures::StreamExt;
use futures::future::ready;
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::Controller;
use kube::runtime::controller::Action;
use kube::runtime::watcher::Config;
use kube::{Api, Client};
use kubera_core::sync::signal::{Receiver, Sender, channel};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinSet;
use tracing::{instrument, warn};

pub fn watch_current_pod_ip_address(
    join_set: &mut JoinSet<()>,
    client: &Client,
    namespace: &str,
    pod_name: &str,
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
        warn!("Ip address for pod {:?}", ip_addr);
        ctx.tx.replace(ip_addr);
        Ok(Action::requeue(Duration::from_secs(60)))
    }

    fn error_policy(_: Arc<Pod>, _: &ControllerError, _: Arc<ControllerContext>) -> Action {
        Action::requeue(Duration::from_secs(5))
    }

    let namespace = namespace.to_string();
    let pod_name = pod_name.to_string();
    let client = client.clone();
    let (tx, rx) = channel(None);

    join_set.spawn(async move {
        let api = Api::<Pod>::namespaced(client, namespace.as_str());
        let config = Config::default().fields(format!("metadata.name={}", pod_name).as_str());
        Controller::new(api, config)
            .shutdown_on_signal()
            .run(reconcile, error_policy, Arc::new(ControllerContext { tx }))
            .filter_map(|x| async move { Some(x) })
            .for_each(|_| ready(()))
            .await;
    });

    rx
}
