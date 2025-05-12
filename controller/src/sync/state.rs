use std::sync::Arc;
use tokio::sync::watch::{
    Receiver as WatchReceiver, Sender as WatchSender, channel as watch_channel,
};

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
    pub fn replace(&self, value: T) -> () {
        if **self.tx.borrow() != value {
            self.tx.send_replace(Arc::new(value));
        }
    }
}

#[derive(Clone)]
pub struct Receiver<T>
where
    T: PartialEq,
{
    rx: WatchReceiver<Arc<T>>,
}

impl<T> Receiver<T>
where
    T: PartialEq,
{
    pub fn current(&self) -> Arc<T> {
        self.rx.borrow().clone()
    }
    pub async fn changed(&mut self) {
        self.rx.changed().await.unwrap();
    }
}
