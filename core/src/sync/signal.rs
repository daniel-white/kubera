use std::sync::Arc;
use anyhow::Result;
use tokio::sync::watch::{
    channel as watch_channel, Receiver as WatchReceiver, Sender as WatchSender,
};
use tracing::trace;

#[derive(Debug)]
pub struct RecvError;

pub fn channel<T>(value: T) -> (Sender<T>, Receiver<T>)
where
    T: PartialEq,
{
    let (tx, rx) = watch_channel(Arc::new(value));
    (Sender { tx }, Receiver { rx })
}

#[derive(Clone)]
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

    pub fn replace(&self, value: T) -> () {
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
    rx: WatchReceiver<Arc<T>>,
}

impl<T> Receiver<T>
where
    T: PartialEq + Clone,
{
    pub fn current(&self) -> Arc<T> {
        self.rx.borrow().clone()
    }

    pub async fn changed(&mut self) -> Result<(), RecvError> {
        self.rx.changed().await.map_err(|_| {
            trace!("Sender dropped");
            RecvError
        })
    }
}
