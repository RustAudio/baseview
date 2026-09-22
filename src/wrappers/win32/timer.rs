use crate::wrappers::win32::window::HWnd;
use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::time::Duration;
use windows_core::Error;
use windows_sys::Win32::Foundation::WPARAM;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct TimerId(NonZeroUsize);

impl TimerId {
    pub fn from_wparam(wparam: WPARAM) -> Option<Self> {
        Some(Self(NonZeroUsize::new(wparam)?))
    }

    pub fn as_raw(&self) -> usize {
        self.0.get()
    }
}

#[derive(PartialEq, Eq)]
pub struct TimerSlot {
    id: Cell<Option<TimerId>>,
    hwnd: HWnd,
}

impl TimerSlot {
    pub fn empty(hwnd: HWnd) -> Self {
        Self { hwnd, id: None.into() }
    }

    pub fn is_running(&self) -> bool {
        todo!()
    }

    pub fn restart(&self, timeout_msec: u32) -> Result<(), Error> {
        todo!()
    }

    pub fn kill(&self) {
        todo!()
    }

    pub fn matches_id(&self, other: TimerId) -> bool {
        match self.id.get() {
            None => false,
            Some(id) => id == other,
        }
    }
}

impl Drop for TimerSlot {
    fn drop(&mut self) {
        todo!()
    }
}

pub struct TimerList {
    timers: RefCell<Vec<TimerId>>,
}

impl TimerList {
    pub fn new() -> Self {
        Self { timers: Vec::new().into() }
    }

    pub fn add_new_timer(&self, window: HWnd, timeout: Duration) -> Result<(), Error> {
        todo!()
    }

    pub fn remove_if_exists(&self, window: HWnd, id: TimerId) -> Result<bool, Error> {
        todo!()
    }

    pub fn destroy_all(&self, window: HWnd) {
        todo!()
    }
}
