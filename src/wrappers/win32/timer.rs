use crate::wrappers::win32::window::HWnd;
use std::cell::RefCell;
use std::num::NonZeroUsize;
use windows_core::Error;
use windows_sys::Win32::Foundation::WPARAM;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct TimerId(NonZeroUsize);

impl TimerId {
    pub fn from_wparam(wparam: WPARAM) -> Option<Self> {
        Some(Self(NonZeroUsize::new(wparam)?))
    }
}

#[derive(PartialEq, Eq)]
pub struct Timer {
    id: TimerId,
    hwnd: HWnd,
}

impl Timer {
    pub fn new(window: HWnd, timeout_msec: u32) -> Result<Self, Error> {
        todo!()
    }

    pub fn reset(&self, timeout_msec: u32) -> Result<(), Error> {
        todo!()
    }

    pub fn id(&self) -> TimerId {
        self.id
    }
}

impl PartialEq<TimerId> for Timer {
    fn eq(&self, other: &TimerId) -> bool {
        self.id == *other
    }
}

impl Drop for Timer {
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

    pub fn add_new_timer(&self, window: HWnd, timeout_msec: u32) -> Result<(), Error> {
        todo!()
    }

    pub fn remove_if_exists(&self, window: HWnd, id: TimerId) -> Result<bool, Error> {
        todo!()
    }

    pub fn destroy_all(&self, window: HWnd) {
        todo!()
    }
}
