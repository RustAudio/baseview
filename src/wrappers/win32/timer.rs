use crate::wrappers::win32::window::HWnd;
use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::time::Duration;
use windows_core::Error;
use windows_sys::Win32::Foundation::WPARAM;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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
        self.id.get().is_some()
    }

    pub fn restart(&self, timeout_msec: u32) {
        self.kill();

        eprintln!("timer start");
        match self.hwnd.set_timer(timeout_msec) {
            Ok(timer_id) => self.id.set(Some(dbg!(timer_id))),
            Err(e) => crate::warn!("Failed to start timer: {}", e),
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

impl Drop for TimerSlot {
    fn drop(&mut self) {
        self.kill()
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
        let new_timer_id = window.set_timer(timeout_msec)?;
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

    pub fn destroy_all(&self, window: HWnd) {
        let timers = self.timers.take();

        for timer_id in timers {
            if let Err(e) = window.kill_timer(timer_id) {
                crate::warn!("Could not remove timer: {e}")
            }
        }
    }
}
