use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use std::sync::Arc;
use tokio::sync::watch::{
    Receiver as WatchReceiver, Sender as WatchSender, channel as watch_channel,
};
use tracing::trace;

#[derive(Debug)]
pub struct RecvError;

pub fn channel<T>(value: T) -> (Sender<T>, Receiver<T>)
where
    T: PartialEq,
{
    let (tx, rx) = watch_channel(Arc::new(value));
    (
        Sender { tx },
        Receiver {
            rx: AtomicRefCell::new(rx),
        },
    )
}

#[derive(Clone, Debug)]
pub struct Sender<T>
where
    T: PartialEq,
{
    tx: WatchSender<Arc<T>>,
}

impl<T> Sender<T>
where
    T: PartialEq,
{
    pub fn current(&self) -> Arc<T> {
        self.tx.borrow().clone()
    }

    pub fn replace(&self, value: T) {
        if *self.tx.borrow().as_ref() != value {
            trace!("Replacing value in signal");
            self.tx.send_replace(Arc::new(value));
        }
    }
}

#[derive(Clone, Debug)]
pub struct Receiver<T>
where
    T: PartialEq,
{
    rx: AtomicRefCell<WatchReceiver<Arc<T>>>,
}

impl<T> Receiver<T>
where
    T: PartialEq + Clone,
{
    pub fn current(&self) -> Arc<T> {
        self.rx.borrow().borrow().clone()
    }

    pub async fn changed(&self) -> Result<(), RecvError> {
        self.rx.borrow_mut().changed().await.map_err(|_| {
            trace!("Sender dropped");
            RecvError
        })
    }
}
