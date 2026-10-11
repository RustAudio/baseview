use crate::platform::x11::host_handle::HostHandle;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod eventfd;
use crate::platform::prelude::PresentStateShared;
use crate::DropData;
use eventfd::*;

// TODO: only instanciate FDs and stuff when first waker is created
pub(crate) struct WakerSource {
    inner: EventFdWakerSource,
    flags: Arc<SharedFlags>,
    redraw_channel_send: SyncSender<Instant>,
    redraw_channel_recv: Receiver<Instant>,
}

impl WakerSource {
    pub fn new(host: &HostHandle) -> Self {
        if host.fd_support().is_some() {
        } else if host.timer_support().is_some() {
            todo!()
        } else {
        }

        let (send, recv) = std::sync::mpsc::sync_channel(100);

        Self {
            inner: EventFdWakerSource::new(),
            flags: Arc::new(SharedFlags::new()),
            redraw_channel_recv: recv,
            redraw_channel_send: send,
        }
    }

    pub fn create_waker(&self) -> WindowWaker {
        WindowWaker {
            inner: self.inner.create_waker(),
            flag: Arc::clone(&self.flags),
            redraw_channel_send: self.redraw_channel_send.clone(),
        }
    }

    pub fn poll_requests(&self, present_state_shared: &PresentStateShared, host: &HostHandle) {
        if !self.inner.poll() {
            return;
        }

        let flags = self.flags.take();
        let requests = self.get_all_redraw_requests();

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

    fn get_all_redraw_requests(&self) -> Vec<Instant> {
        let mut requests = vec![];

        loop {
            match self.redraw_channel_recv.try_recv() {
                Ok(instant) => requests.push(instant),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => unreachable!(), // We always hold onto one sender
            }
        }

        requests
    }

    pub fn inner_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

#[derive(Clone)]
pub struct WindowWaker {
    inner: EventFdWaker,
    flag: Arc<SharedFlags>,
    redraw_channel_send: SyncSender<Instant>,
}

impl WindowWaker {
    pub fn request_redraw(&self) {
        self.flag.set_redraw();
        self.inner.wake();
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        if duration.is_zero() {
            return self.request_redraw();
        }

        self.redraw_channel_send.send(Instant::now() + duration).unwrap();
        self.inner.wake()
    }

    pub fn request_poll(&self) {
        self.flag.set_poll();
        self.inner.wake();
    }
}

impl Drop for WindowWaker {
    fn drop(&mut self) {
        self.flag.set_dropped();
    }
}

struct SharedFlags(AtomicU8);

impl SharedFlags {
    pub fn new() -> Self {
        Self(AtomicU8::new(0))
    }

    const POLL: u8 = 1 << 0;
    const REDRAW: u8 = 1 << 1;
    const DROPPED: u8 = 1 << 2;

    fn set_poll(&self) {
        self.0.fetch_or(Self::POLL, Ordering::Release);
    }

    fn set_redraw(&self) {
        self.0.fetch_or(Self::REDRAW, Ordering::Release);
    }

    fn set_dropped(&self) {
        self.0.fetch_or(Self::DROPPED, Ordering::Release);
    }

    fn take(&self) -> FlagsValues {
        FlagsValues(self.0.swap(0, Ordering::Acquire))
    }
}

pub struct FlagsValues(u8);

impl FlagsValues {
    fn poll(&self) -> bool {
        self.0 & SharedFlags::POLL != 0
    }

    fn redraw(&self) -> bool {
        self.0 & SharedFlags::REDRAW != 0
    }

    fn dropped(&self) -> bool {
        self.0 & SharedFlags::DROPPED != 0
    }
}
