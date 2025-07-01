use kube::Client;
use kubera_core::sync::signal::{Receiver, channel};
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

impl AsRef<Client> for KubeClientCell {
    fn as_ref(&self) -> &Client {
        &self.0
    }
}

impl KubeClientCell {
    pub fn cloned(&self) -> Client {
       self.0.clone()
    }
}

pub fn start_kubernetes_client(join_set: &mut JoinSet<()>) -> Receiver<Option<KubeClientCell>> {
    let (tx, rx) = channel(None);

    join_set.spawn(async move {
        match Client::try_default().await {
            Ok(client) => {
                let client_cell = KubeClientCell(client);
                tx.replace(Some(client_cell));
            }
            Err(e) => error!("Failed to create Kubernetes client: {}", e),
        };
    });

    rx
}
