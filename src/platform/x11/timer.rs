use crate::platform::x11::event_loop::EventLoop;
use crate::platform::PlatformError;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};
use std::rc::{Rc, Weak};
use std::time::Duration;

pub type TimerHandle = Rc<TimerHandleInner>;

#[derive(PartialEq, Eq)]
pub struct TimerHandleInner {
    token: RegistrationToken,
}

impl TimerHandleInner {}

pub(crate) fn insert_timer(
    loop_handle: &LoopHandle<EventLoop>, duration: Duration,
) -> Result<TimerHandle, PlatformError> {
    let timer = Timer::from_duration(duration);

    let handle = Rc::new_cyclic(move |this| {
        let this = Weak::clone(this);

        let result = loop_handle.insert_source(timer, move |_, _, e| {
            if let Some(this) = this.upgrade() {
                e.handle_timer(&this);
            }
            TimeoutAction::ToDuration(duration)
        });

        match result {
            Err(e) => {
                panic!("Failed to insert timer: {:?}", e);
            }
            Ok(token) => TimerHandleInner { token },
        }
    });

    Ok(handle)
}
