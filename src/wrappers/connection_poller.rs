use polling::{Event, Events, Poller};
use std::error::Error;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::{Duration, Instant};

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
        unsafe { poller.add(&fd, Event::readable(CONNECTION_KEY))? };

        Ok(Self { poller, fd, events: Events::new() })
    }

    pub fn wait(&mut self, deadline: Instant) -> io::Result<PollStatus> {
        self.events.clear();
        // NOTE: polling crate already handles retrying on EINTR
        let new_events_count = self.poller.wait_deadline(&mut self.events, deadline)?;

        if new_events_count == 0 {
            return Ok(PollStatus::Nothing);
        }

        for event in self.events.iter() {
            if event.key != CONNECTION_KEY {
                continue;
            }

            if let Some(true) = event.is_err() {
                panic!("xcb connection poll error")
            }

            if event.is_interrupt() {
                return Ok(PollStatus::ConnectionClosed);
            }
        }

        Ok(PollStatus::Nothing)
    }

    pub fn delete(self) -> Result<(), Box<dyn Error>> {
        Ok(self.poller.delete(self.fd)?)
    }
}

impl<'a> Drop for ConnectionPoller<'a> {
    fn drop(&mut self) {
        let _ = self.poller.delete(self.fd);
    }
}

pub enum PollStatus {
    Nothing,
    ReadAvailable,
    ConnectionClosed,
}
