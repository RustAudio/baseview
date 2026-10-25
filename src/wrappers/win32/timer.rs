use crate::wrappers::win32::window::HWnd;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::ptr::null_mut;
use std::time::Duration;
use windows_core::Error;
use windows_sys::Win32::Foundation::{FALSE, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TimerId(NonZeroUsize);

impl TimerId {
    pub fn from_raw(wparam: WPARAM) -> Option<Self> {
        Some(Self(NonZeroUsize::new(wparam)?))
    }

    pub fn as_raw(&self) -> usize {
        self.0.get()
    }

    pub const fn from_const(value: usize) -> Self {
        let Some(value) = NonZeroUsize::new(value) else { unreachable!() };
        Self(value)
    }
}

pub trait TimerType {
    const ID: TimerId;
    const TIMEOUT_MSEC: u32;
}

#[derive(PartialEq, Eq)]
pub struct TimerSlot<T> {
    running: Cell<bool>,
    hwnd: HWnd,
    _timer: PhantomData<T>,
}

impl<T: TimerType> TimerSlot<T> {
    pub fn empty(hwnd: HWnd) -> Self {
        Self { hwnd, running: false.into(), _timer: PhantomData }
    }

    pub fn is_running(&self) -> bool {
        self.running.get()
    }

    pub fn start_if_not_running(&self) {
        if !self.is_running() {
            self.start()
        }
    }

    fn start(&self) {
        match self.hwnd.reset_timer(T::ID, T::TIMEOUT_MSEC) {
            Ok(()) => self.running.set(true),
            Err(e) => crate::warn!("Failed to start timer: {}", e),
        }
    }

    pub fn set_running(&self, running: bool) {
        match (running, self.is_running()) {
            (true, true) | (false, false) => (), // Nothing to do
            (false, true) => self.kill(),
            (true, false) => self.start(),
        }
    }

    pub fn kill(&self) {
        if self.running.take() {
            if let Err(e) = self.hwnd.kill_timer(T::ID) {
                crate::warn!("Failed to kill timer: {}", e);
            }
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

    pub fn add_new_timer(&self, timeout: Duration) -> Result<TimerId, Error> {
        let timeout_msec = timeout.as_millis().try_into().unwrap_or(u32::MAX);
        let new_timer_id = create_thread_timer(timeout_msec)?;
        self.timers.borrow_mut().push(new_timer_id);
        Ok(new_timer_id)
    }

    pub fn exists(&self, timer_id: TimerId) -> bool {
        self.timers.borrow().contains(&timer_id)
    }

    pub fn remove_if_exists(&self, id: TimerId) -> Result<bool, Error> {
        if !self.pop_if_exists(id) {
            return Ok(false);
        };

        kill_thread_timer(id)?;

        Ok(true)
    }

    fn pop_if_exists(&self, id: TimerId) -> bool {
        let mut timers = self.timers.borrow_mut();
        let Some(index) = timers.iter().position(|&t| t == id) else { return false };
        timers.swap_remove(index);
        true
    }

    fn pop(&self) -> Option<TimerId> {
        self.timers.borrow_mut().pop()
    }

    fn stop_and_destroy_all(&self) {
        while let Some(timer) = self.pop() {
            if let Err(e) = kill_thread_timer(timer) {
                crate::warn!("Failed to kill timer: {}", e);
            }
        }
    }
}

impl Drop for TimerList {
    fn drop(&mut self) {
        self.stop_and_destroy_all()
    }
}

fn create_thread_timer(elapse: u32) -> windows_core::Result<TimerId> {
    let result = unsafe { SetTimer(null_mut(), 0, elapse, None) };

    let Some(id) = TimerId::from_raw(result) else {
        return Err(Error::from_thread());
    };

    Ok(id)
}

fn kill_thread_timer(timer_id: TimerId) -> windows_core::Result<()> {
    let result = unsafe { KillTimer(null_mut(), timer_id.as_raw()) };

    if result == FALSE {
        return Err(Error::from_thread());
    }

    Ok(())
}
