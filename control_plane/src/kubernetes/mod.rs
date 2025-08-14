use kube::Client;
use std::ops::Deref;
use tracing::error;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

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

pub fn start_kubernetes_client(task_builder: &TaskBuilder) -> Receiver<KubeClientCell> {
    let (tx, rx) = signal("kubernetes_client");

    task_builder
        .new_task("kubernetes_client")
        .spawn(async move {
            match Client::try_default().await {
                Ok(client) => {
                    let client_cell = KubeClientCell(client);
                    tx.set(client_cell).await;
                    let tx = Box::new(tx);
                    Box::leak(tx); // Leak the sender to keep it alive
                }
                Err(e) => error!("Failed to create Kubernetes client: {}", e),
            }
        });

    rx
}
