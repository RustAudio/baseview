use polling::{Event, Events, PollMode, Poller};
use std::io;
use std::io::ErrorKind;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Instant;

pub struct ConnectionPoller<'a> {
    poller: Poller,
    events: Events,
    fd: BorrowedFd<'a>,
}

const CONNECTION_KEY: usize = 42;

impl<'a> ConnectionPoller<'a> {
    pub fn new(source: &'a impl AsFd) -> io::Result<Self> {
        let poller = Poller::new()?;
        let fd = source.as_fd();
        unsafe { poller.add_with_mode(&fd, Event::readable(CONNECTION_KEY), PollMode::Level)? };

        Ok(Self { poller, fd, events: Events::new() })
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
            if event.key != CONNECTION_KEY {
                continue;
            }

            if let Some(true) = event.is_err() {
                return Err(io::Error::new(ErrorKind::BrokenPipe, "X11 connection closed"));
            }

            if event.is_interrupt() {
                return Ok(PollStatus::ConnectionClosed);
            }

            return Ok(PollStatus::ReadAvailable);
        }

        Ok(PollStatus::Nothing)
    }

    pub fn delete(self) -> io::Result<()> {
        self.poller.delete(self.fd)
    }
}

impl<'a> Drop for ConnectionPoller<'a> {
    fn drop(&mut self) {
        let _ = self.poller.delete(self.fd);
    }
}

#[derive(Debug)]
pub enum PollStatus {
    Nothing,
    ReadAvailable,
    ConnectionClosed,
}
