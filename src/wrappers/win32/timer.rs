use crate::wrappers::win32::window::HWnd;
use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::time::Duration;
use windows_core::Error;
use windows_sys::Win32::Foundation::WPARAM;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TimerId(NonZeroUsize);

impl TimerId {
    pub fn from_raw(wparam: WPARAM) -> Option<Self> {
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
        self.id.get().is_some()
    }

    pub fn start_if_not_running(&self, timeout_msec: u32) {
        if !self.is_running() {
            self.start(timeout_msec);
        }
    }

    fn start(&self, timeout_msec: u32) {
        match self.hwnd.create_timer(timeout_msec) {
            Ok(timer_id) => self.id.set(Some(timer_id)),
            Err(e) => crate::warn!("Failed to start timer: {}", e),
        }
    }

    pub fn set_running(&self, running: bool, timeout_msec: u32) {
        match (running, self.is_running()) {
            (true, true) | (false, false) => (), // Nothing to do
            (false, true) => self.kill(),
            (true, false) => self.start(timeout_msec),
        }
    }

    pub fn kill(&self) {
        if let Some(timer_id) = self.id.take() {
            if let Err(e) = self.hwnd.kill_timer(timer_id) {
                crate::warn!("Failed to kill timer: {}", e);
            }
        }
    }

    pub fn matches_id(&self, other: TimerId) -> bool {
        match self.id.get() {
            None => false,
            Some(id) => id == other,
        }
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
        let timeout_msec = timeout.as_millis().try_into().unwrap_or(u32::MAX);
        let new_timer_id = window.create_timer(timeout_msec)?;
        self.timers.borrow_mut().push(new_timer_id);
        Ok(())
    }

    pub fn remove_if_exists(&self, window: HWnd, id: TimerId) -> Result<bool, Error> {
        if !self.pop_if_exists(id) {
            return Ok(false);
        };

        window.kill_timer(id)?;

        Ok(true)
    }

    fn pop_if_exists(&self, id: TimerId) -> bool {
        let mut timers = self.timers.borrow_mut();
        let Some(index) = timers.iter().position(|&t| t == id) else { return false };
        timers.swap_remove(index);
        true
    }
}
