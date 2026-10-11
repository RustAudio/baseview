use rustix::event::{eventfd, EventfdFlags};
use rustix::io::{read, write, Errno};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

pub struct EventFdWakerSource {
    fd: Arc<OwnedFd>,
}

impl EventFdWakerSource {
    pub fn new() -> Self {
        let fd = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK).unwrap();
        Self { fd: Arc::new(fd) }
    }

    pub fn create_waker(&self) -> EventFdWaker {
        EventFdWaker(Arc::clone(&self.fd))
    }

    // Returns true if something requested this to be woken up
    pub fn poll(&self) -> bool {
        // The eventfd counter is effectively a u64.
        const BYTES: usize = 8;
        let mut buf = [0u8; BYTES];

        match read(&self.fd, &mut buf) {
            // Reading from an eventfd should only ever produce 8 bytes. No looping
            // is required.
            Ok(BYTES) => u64::from_ne_bytes(buf) > 0,

            Ok(_) => unreachable!(),

            // Any other error can be propagated.
            Err(e) => {
                crate::warn!("Failed to read from waker's eventfd counter: {}", e);
                false
            }
        }
    }
}

impl AsFd for EventFdWakerSource {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[derive(Clone)]
pub struct EventFdWaker(Arc<OwnedFd>);

impl EventFdWaker {
    pub fn wake(&self) {
        match write(&self.0, &1u64.to_ne_bytes()) {
            Ok(_) => {}
            // Counter hit capacity. We can ignore it as that means the loop will be already woken up by *something*
            Err(Errno::AGAIN) => {}

            Err(e) => {
                crate::warn!("Failed to write to waker's eventfd counter: {}", e)
            }
        }
    }
}
