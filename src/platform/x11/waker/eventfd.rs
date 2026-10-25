use rustix::event::{eventfd, EventfdFlags};
use rustix::io::{read, write, Errno};
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

pub struct EventFd(OwnedFd);

impl EventFd {
    pub fn new() -> io::Result<Self> {
        Ok(Self(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?))
    }

    // Returns true if something requested this to be woken up
    pub fn poll(&self) -> bool {
        // The eventfd counter is effectively a u64.
        const BYTES: usize = 8;
        let mut buf = [0u8; BYTES];

        match read(&self.0, &mut buf) {
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

impl AsFd for EventFd {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
