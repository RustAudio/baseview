use polling::{Event, Events, PollMode, Poller};
use std::io;
use std::io::ErrorKind;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Instant;

pub struct ConnectionPoller<'a> {
    poller: Poller,
    events: Events,
    fd: BorrowedFd<'a>,
    waker_fd: Option<BorrowedFd<'a>>,
}

const CONNECTION_KEY: usize = 42;
const WAKER_KEY: usize = 69;

impl<'a> ConnectionPoller<'a> {
    pub fn new(source: &'a impl AsFd, waker_fd: Option<BorrowedFd<'a>>) -> io::Result<Self> {
        let poller = Poller::new()?;

        let fd = source.as_fd();
        unsafe { poller.add_with_mode(&fd, Event::readable(CONNECTION_KEY), PollMode::Level)? };

        if let Some(waker_fd) = waker_fd {
            unsafe {
                poller.add_with_mode(&waker_fd, Event::readable(WAKER_KEY), PollMode::Level)?
            };
        }

        Ok(Self { poller, fd, events: Events::new(), waker_fd })
    }

    pub fn wait(&mut self, deadline: Option<Instant>) -> io::Result<PollStatus> {
        self.events.clear();

        // NOTE: polling crate already handles retrying on EINTR
        let new_events_count = match deadline {
            Some(deadline) => self.poller.wait_deadline(&mut self.events, deadline)?,
            None => self.poller.wait(&mut self.events, None)?,
        };

        if new_events_count == 0 {
            return Ok(PollStatus::Nothing);
        }

        for event in self.events.iter() {
            match event.key {
                CONNECTION_KEY => {
                    if let Some(true) = event.is_err() {
                        return Err(io::Error::new(ErrorKind::BrokenPipe, "X11 connection closed"));
                    }

                    if event.is_interrupt() {
                        return Ok(PollStatus::ConnectionClosed);
                    }

                    return Ok(PollStatus::ReadAvailable);
                }
                WAKER_KEY => {
                    if let Some(true) = event.is_err() {
                        return Err(io::Error::new(ErrorKind::BrokenPipe, "X11 connection closed"));
                    }

                    return Ok(PollStatus::WakerTriggered);
                }
                _ => continue,
            }
        }

        Ok(PollStatus::Nothing)
    }

    pub fn delete(self) -> io::Result<()> {
        if let Some(waker_fd) = self.waker_fd {
            let _ = self.poller.delete(waker_fd);
        }

        self.poller.delete(self.fd)
    }
}

impl<'a> Drop for ConnectionPoller<'a> {
    fn drop(&mut self) {
        if let Some(waker_fd) = self.waker_fd {
            let _ = self.poller.delete(waker_fd);
        }

        let _ = self.poller.delete(self.fd);
    }
}

#[derive(Debug)]
pub enum PollStatus {
    Nothing,
    ReadAvailable,
    WakerTriggered,
    ConnectionClosed,
}
