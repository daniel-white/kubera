use anyhow::Result;
use tokio::sync::watch::{
    channel as watch_channel, Receiver as WatchReceiver, Sender as WatchSender,
};
use tracing::trace;

#[derive(Debug, Clone)]
pub struct RecvError;

pub fn channel<T>(value: T) -> (Sender<T>, Receiver<T>)
where
    T: PartialEq + Clone,
{
    let (tx, rx) = watch_channel(value);
    (Sender { tx }, Receiver { rx })
}

#[derive(Clone)]
pub struct Sender<T>
where
    T: PartialEq + Clone,
{
    tx: WatchSender<T>,
}

impl<T> Sender<T>
where
    T: PartialEq + Clone,
{
    pub fn current(&self) -> T {
        self.tx.borrow().clone()
    }

    pub fn replace(&self, value: T) -> () {
        if *self.tx.borrow() != value {
            trace!("Replacing value in signal");
            self.tx.send_replace(value);
        }
    }
}

#[derive(Clone)]
pub struct Receiver<T>
where
    T: PartialEq + Clone,
{
    rx: WatchReceiver<T>,
}

impl<T> Receiver<T>
where
    T: PartialEq + Clone,
{
    pub fn current(&mut self) -> T {
        self.rx.borrow().clone()
    }

    pub async fn changed(&mut self) -> Result<(), RecvError> {
        self.rx.changed().await.map_err(|_| {
            trace!("Sender dropped");
            RecvError
        })
    }
}
