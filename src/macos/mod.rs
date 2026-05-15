mod keyboard;
mod view;
mod window;

use objc2::rc::Retained;
use objc2::Message;
use std::cell::RefCell;
pub use window::*;

pub struct RetainedCell<T> {
    inner: RefCell<Option<Retained<T>>>,
}

impl<T> RetainedCell<T> {
    pub const fn empty() -> Self {
        RetainedCell { inner: RefCell::new(None) }
    }

    pub const fn new(value: Retained<T>) -> Self {
        RetainedCell { inner: RefCell::new(Some(value)) }
    }

    pub const fn with(value: Option<Retained<T>>) -> Self {
        RetainedCell { inner: RefCell::new(value) }
    }

    pub fn take(&self) -> Option<Retained<T>> {
        self.inner.borrow_mut().take()
    }
}

impl<T: Message> RetainedCell<T> {
    pub fn get(&self) -> Option<Retained<T>> {
        (*self.inner.borrow()).as_ref().map(|inner| inner.retain())
    }

    pub fn set(&self, value: Retained<T>) {
        self.inner.replace(Some(value));
    }
}
