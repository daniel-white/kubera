use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use std::ops::Deref;
use thiserror::Error;
use tokio::sync::watch::{
    channel as watch_channel, Receiver as WatchReceiver, Ref as WatchRef, Sender as WatchSender,
};
use tracing::trace;

#[derive(Debug, Error)]
#[error("Receiver error")]
pub struct RecvError;

#[derive(Debug)]
pub struct Ref<'a, T: PartialEq>(WatchRef<'a, Option<T>>);

unsafe impl<T: PartialEq> Send for Ref<'_, T> {}

impl<T: PartialEq> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("Invalid dereference: value is None")
    }
}

pub fn signal<T: PartialEq>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = watch_channel(None);
    (
        Sender { tx: tx.clone() },
        Receiver {
            tx,
            rx: AtomicRefCell::new(rx),
        },
    )
}

#[derive(Debug)]
pub struct Sender<T: PartialEq> {
    tx: WatchSender<Option<T>>,
}

impl<T: PartialEq> Sender<T> {
    pub fn get(&self) -> Option<Ref<'_, T>> {
        let watch_ref = self.tx.borrow();
        match watch_ref.as_ref() {
            Some(_) => Some(Ref(watch_ref)),
            None => None,
        }
    }

    pub fn set(&self, value: T) {
        match self.get().as_deref() {
            Some(old_value) if old_value != &value => {
                trace!("Replacing value in signal");
                self.tx.send_replace(Some(value));
            }
            None => {
                trace!("Setting value in signal");
                self.tx.send_replace(Some(value));
            }
            _ => trace!("No change in value, not updating signal"),
        }
    }

    pub fn clear(&self) {
        if self.get().is_some() {
            trace!("Clearing value in signal");
            self.tx.send_replace(None);
        } else {
            trace!("No value to clear in signal");
        }
    }

    pub fn replace(&self, value: Option<T>) {
        if self.get().as_deref() != value.as_ref() {
            trace!("Replacing value in signal");
            self.tx.send_replace(value);
        } else {
            trace!("No change in value, not updating signal");
        }
    }
}

#[derive(Clone, Debug)]
pub struct Receiver<T: PartialEq> {
    tx: WatchSender<Option<T>>,
    rx: AtomicRefCell<WatchReceiver<Option<T>>>,
}

impl<T: PartialEq> Receiver<T> {
    pub fn get(&self) -> Option<Ref<'_, T>> {
        let watch_ref = self.tx.borrow();
        match watch_ref.as_ref() {
            Some(_) => Some(Ref(watch_ref)),
            None => None,
        }
    }

    pub async fn changed(&self) -> Result<(), RecvError> {
        self.rx.borrow_mut().changed().await.map_err(|_| {
            trace!("Sender dropped");
            RecvError
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signal_channel() {
        let (tx, rx) = signal();
        assert_eq!(rx.get(), None);

        tx.set(43);
        assert_eq!(rx.get(), Some(&43));

        tx.clear();
        assert_eq!(rx.get(), None);

        tx.set(44);
        assert_eq!(rx.get(), Some(&44));

        tx.set(44); // No change, should not update
        assert_eq!(rx.get(), Some(&44));
    }
}
