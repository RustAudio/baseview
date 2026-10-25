use crate::platform::{BaseviewWindow, PlatformError};
use crate::wrappers::win32::window::{create_window, use_window, HWnd, WindowImpl};
use crate::wrappers::win32::{DpiAwarenessGuard, WindowStyle};
use dpi::PhysicalSize;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::ptr::null_mut;
use std::rc::Rc;
use std::time::Duration;
use windows::Win32::UI::WindowsAndMessaging::WM_TIMER;
use windows_core::{Error, HSTRING};
use windows_sys::Win32::Foundation::{FALSE, LPARAM, LRESULT, WPARAM};
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
    timer_window: Cell<Option<HWnd>>,
    timer_fn: fn(&BaseviewWindow, TimerId),
}

impl TimerList {
    pub fn new(timer_fn: fn(&BaseviewWindow, TimerId)) -> Self {
        Self { timers: Vec::new().into(), timer_window: None.into(), timer_fn }
    }

    fn get_or_create_timer_window(&self, baseview_window: HWnd) -> Result<HWnd, Error> {
        if let Some(timer_window) = self.timer_window.get() {
            return Ok(timer_window);
        }

        let timer_fn = self.timer_fn;
        let timer_window = create_window(
            &HSTRING::new(),
            WindowStyle::dummy(),
            PhysicalSize::new(1, 1),
            None,
            &DpiAwarenessGuard::dummy(),
            move |_| TimerWindow { baseview_window, timer_fn },
        )?;
        self.timer_window.set(Some(timer_window));

        Ok(timer_window)
    }

    pub fn get_new_timer_id(&self) -> TimerId {
        let Some(mut id) = NonZeroUsize::new(1) else { unreachable!() };

        let timers = self.timers.borrow();

        while timers.contains(&TimerId(id)) {
            let Some(new_id) = id.checked_add(1) else { panic!("Too many timers") };
            id = new_id;
        }

        TimerId(id)
    }

    pub fn add_new_timer(
        &self, baseview_window: HWnd, timeout: Duration,
    ) -> Result<TimerId, Error> {
        let timeout_msec = timeout.as_millis().try_into().unwrap_or(u32::MAX);
        let timer_window = self.get_or_create_timer_window(baseview_window)?;
        let new_timer_id = self.get_new_timer_id();

        timer_window.reset_timer(new_timer_id, timeout_msec)?;

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

        let Some(window) = self.timer_window.get() else { unreachable!() };
        window.kill_timer(id)?;

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
        let Some(window) = self.timer_window.get() else { return };

        while let Some(timer) = self.pop() {
            if let Err(e) = window.kill_timer(timer) {
                crate::warn!("Failed to kill timer: {}", e);
            }
        }

        let _ = window.destroy();
        self.timer_window.set(None);
    }
}

impl Drop for TimerList {
    fn drop(&mut self) {
        self.stop_and_destroy_all()
    }
}

struct TimerWindow {
    baseview_window: HWnd,
    timer_fn: fn(&BaseviewWindow, TimerId),
}

impl WindowImpl for TimerWindow {
    fn non_client_create(&self, _: HWnd) -> Result<(), PlatformError> {
        Ok(())
    }

    fn after_create(&self, _: HWnd) -> Result<(), PlatformError> {
        Ok(())
    }

    unsafe fn handle_message(
        &self, _: HWnd, message_code: u32, wparam: WPARAM, _: LPARAM,
    ) -> Option<LRESULT> {
        if message_code != WM_TIMER {
            return None;
        }

        let timer_id = TimerId::from_raw(wparam)?;

        // SAFETY: baseview_window is of type BaseviewWindow
        unsafe {
            use_window::<BaseviewWindow>(self.baseview_window, |hwnd| {
                (self.timer_fn)(hwnd, timer_id)
            })
        };

        Some(0)
    }

    fn before_destroy(&self, _: HWnd) {}
}
