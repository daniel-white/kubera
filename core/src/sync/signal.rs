use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender, channel};
use tracing::trace;

#[derive(Debug, Error)]
#[error("Receiver error")]
pub struct RecvError;

pub fn signal<T: PartialEq + Clone>() -> (Sender<T>, Receiver<T>) {
    let data = Arc::new(RwLock::new(None));
    let (tx, rx) = channel(10);
    (
        Sender {
            data: data.clone(),
            tx: tx.clone(),
        },
        Receiver {
            tx,
            rx: AtomicRefCell::new(rx),
            data,
        },
    )
}

#[derive(Clone, Debug)]
pub struct Sender<T: PartialEq + Clone> {
    data: Arc<RwLock<Option<T>>>,
    tx: BroadcastSender<()>,
}

impl<T: PartialEq + Clone> Sender<T> {
    pub async fn get(&self) -> Option<T> {
        self.data.read().await.clone()
    }

    pub async fn set(&self, value: T) {
        let value = match self.data.read().await.as_ref() {
            Some(old_value) if old_value != &value => {
                trace!("Replacing value in signal");
                Some(value)
            }
            None => {
                trace!("Setting value in signal");
                Some(value)
            }
            _ => {
                trace!("No change in value, not updating signal");
                None
            }
        };

        if let Some(value) = value {
            self.data.write().await.replace(value);
            let _ = self.tx.send(());
        }
    }

    pub async fn clear(&self) {
        if self.data.read().await.is_some() {
            trace!("Clearing value in signal");
            self.data.write().await.take();
            let _ = self.tx.send(());
        } else {
            trace!("No value to clear in signal");
        }
    }

    pub async fn replace(&self, value: Option<T>) {
        if self.data.read().await.as_ref() == value.as_ref() {
            trace!("No change in value, not updating signal");
        } else if let Some(value) = value {
            trace!("Replacing value in signal");
            self.data.write().await.replace(value);
            let _ = self.tx.send(());
        } else {
            trace!("Clearing value in signal");
            self.data.write().await.take();
            let _ = self.tx.send(());
        }
    }
}

#[derive(Debug)]
pub struct Receiver<T: PartialEq + Clone> {
    tx: BroadcastSender<()>,
    rx: AtomicRefCell<BroadcastReceiver<()>>,
    data: Arc<RwLock<Option<T>>>,
}

impl<T: PartialEq + Clone> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        let rx = self.tx.subscribe();
        Receiver {
            tx: self.tx.clone(),
            rx: AtomicRefCell::new(rx),
            data: self.data.clone(),
        }
    }
}

impl<T: PartialEq + Clone> Receiver<T> {
    pub async fn get(&self) -> Option<T> {
        self.data.read().await.clone()
    }

    pub async fn changed(&self) -> Result<(), RecvError> {
        self.rx.borrow_mut().recv().await.map_err(|_| {
            trace!("Sender dropped");
            RecvError
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_signal_channel() {
        let (tx, rx) = signal();
        assert_eq!(rx.get().await, None);

        tx.set(43).await;
        assert_eq!(rx.get().await, Some(43));

        tx.clear().await;
        assert_eq!(rx.get().await, None);

        tx.set(44).await;
        assert_eq!(rx.get().await, Some(44));

        tx.set(44).await; // No change, should not update
        assert_eq!(rx.get().await, Some(44));
    }
}
