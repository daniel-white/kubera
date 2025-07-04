use kube::Client;
use kubera_core::sync::signal::{Receiver, channel};
use std::ops::Deref;
use tokio::task::JoinSet;
use tracing::error;

pub mod objects;

#[derive(Clone)]
pub struct KubeClientCell(Client);

impl PartialEq for KubeClientCell {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Deref for KubeClientCell {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<KubeClientCell> for Client {
    fn from(client_cell: KubeClientCell) -> Self {
        client_cell.0
    }
}

pub fn start_kubernetes_client(join_set: &mut JoinSet<()>) -> Receiver<Option<KubeClientCell>> {
    let (tx, rx) = channel(None);

    join_set.spawn(async move {
        match Client::try_default().await {
            Ok(client) => {
                let client_cell = KubeClientCell(client);
                tx.replace(Some(client_cell));
                let tx = Box::new(tx);
                Box::leak(tx); // Leak the sender to keep it alive
            }
            Err(e) => error!("Failed to create Kubernetes client: {}", e),
        };
    });

    rx
}
