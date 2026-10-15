use crate::platform::x11::host_handle::HostHandle;
use std::cell::RefCell;
use std::mem;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod eventfd;
use crate::platform::prelude::PresentStateShared;
use crate::DropData;
use eventfd::*;

pub(crate) struct WakerSource {
    shared: RefCell<Option<Arc<Shared>>>,
}

impl WakerSource {
    pub fn new() -> Self {
        Self { shared: None.into() }
    }

    fn get_shared_or_create(&self) -> Arc<Shared> {
        let mut shared = self.shared.borrow_mut();
        if let Some(shared) = &*shared {
            return Arc::clone(shared);
        }

        let new_shared = Arc::new(Shared::new());
        *shared = Some(Arc::clone(&new_shared));
        new_shared
    }

    pub fn create_waker(&self) -> WindowWaker {
        WindowWaker { shared: self.get_shared_or_create() }
    }

    pub fn poll_requests(&self, present_state_shared: &PresentStateShared, host: &HostHandle) {
        let Some(shared) = &*self.shared.borrow() else { return };

        if let Some(event_fd) = &shared.event_fd {
            if !event_fd.poll() {
                return;
            }
        }

        let flags = shared.take_flags();
        let requests = shared.take_requests();

        for instant in requests {
            if let Some(duration) = instant.checked_duration_since(Instant::now()) {
                present_state_shared.request_present_notify_after(duration, host);
            } else {
                present_state_shared.request_present_notify()
            }
        }

        if flags.poll() {
            present_state_shared.request_poll();
        }

        if flags.redraw() {
            present_state_shared.request_present_notify();
        }

        if flags.dropped() {
            // TODO
        }
    }

    pub fn inner_fd(&self) -> Option<Arc<Shared>> {
        let Some(shared) = &*self.shared.borrow() else { return None };
        Some(Arc::clone(&shared))
    }
}

#[derive(Clone)]
pub struct WindowWaker {
    shared: Arc<Shared>,
}

impl WindowWaker {
    fn wake(&self) {
        if let Some(event_fd) = &self.shared.event_fd {
            event_fd.wake()
        }
    }

    pub fn request_redraw(&self) {
        self.shared.set_redraw();
        self.wake();
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        if duration.is_zero() {
            return self.request_redraw();
        }

        // If this overflows, then this is so long in the future that it will realistically not occur anyway
        let Some(instant) = Instant::now().checked_add(duration) else {
            return;
        };

        self.shared.add_request(instant);
        self.wake()
    }

    pub fn request_poll(&self) {
        self.shared.set_poll();
        self.wake();
    }
}

impl Drop for WindowWaker {
    fn drop(&mut self) {
        self.shared.set_dropped();
    }
}

pub(crate) struct Shared {
    flags: AtomicU8,
    requests: Mutex<Vec<Instant>>,
    event_fd: Option<EventFd>,
}

impl Shared {
    pub fn new() -> Self {
        let event_fd = match EventFd::new() {
            Ok(fd) => Some(fd),
            Err(e) => {
                crate::warn!("Could not create WindowWaker's eventfd: {}", e);
                None
            }
        };

        Self { flags: AtomicU8::new(0), requests: Mutex::new(vec![]), event_fd }
    }

    pub(crate) fn fd(&self) -> Option<BorrowedFd<'_>> {
        Some(self.event_fd.as_ref()?.as_fd())
    }

    const POLL: u8 = 1 << 0;
    const REDRAW: u8 = 1 << 1;
    const DROPPED: u8 = 1 << 2;

    fn set_poll(&self) {
        self.flags.fetch_or(Self::POLL, Ordering::Release);
    }

    fn set_redraw(&self) {
        self.flags.fetch_or(Self::REDRAW, Ordering::Release);
    }

    fn set_dropped(&self) {
        self.flags.fetch_or(Self::DROPPED, Ordering::Release);
    }

    fn take_flags(&self) -> FlagsValues {
        FlagsValues(self.flags.swap(0, Ordering::Acquire))
    }

    fn add_request(&self, instant: Instant) {
        const MAX_REQUESTS: usize = 10_000;
        let mut requests = self.requests.lock().unwrap_or_else(|e| e.into_inner());
        if requests.len() >= MAX_REQUESTS {
            return;
        }

        requests.push(instant);
    }

    fn take_requests(&self) -> Vec<Instant> {
        let mut requests = self.requests.lock().unwrap_or_else(|e| e.into_inner());
        mem::take(&mut requests)
    }
}

pub struct FlagsValues(u8);

impl FlagsValues {
    fn poll(&self) -> bool {
        self.0 & Shared::POLL != 0
    }

    fn redraw(&self) -> bool {
        self.0 & Shared::REDRAW != 0
    }

    fn dropped(&self) -> bool {
        self.0 & Shared::DROPPED != 0
    }
}
