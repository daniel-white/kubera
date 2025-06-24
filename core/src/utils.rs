use std::ops::{Deref, DerefMut};

pub struct DropTracker<T>(T);

impl<T> DropTracker<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for DropTracker<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl <T> DerefMut for DropTracker<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(debug_assertions)]
impl<T> Drop for DropTracker<T> {
    fn drop(&mut self) {
        panic!("Value of type {} was dropped", std::any::type_name::<T>());
    }
}
