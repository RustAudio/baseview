use crate::platform::x11::event_loop::EventLoop;
use crate::platform::PlatformError;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};
use std::rc::{Rc, Weak};
use std::time::Duration;

#[derive(Clone, Eq)]
pub struct TimerHandle(Rc<TimerHandleInner>);

impl PartialEq for TimerHandle {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(PartialEq, Eq)]
pub struct TimerHandleInner {
    token: RegistrationToken,
}

impl TimerHandleInner {}

pub(crate) fn insert_timer(
    loop_handle: &LoopHandle<EventLoop>, duration: Duration,
) -> Result<TimerHandle, PlatformError> {
    let timer = Timer::from_duration(duration);

    let handle = Rc::<TimerHandleInner>::new_cyclic(move |this| {
        let this = Weak::clone(this);

        let result = loop_handle.insert_source(timer, move |_, _, e| {
            if let Some(this) = this.upgrade() {
                e.handle_timer(&TimerHandle(this));
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

    Ok(TimerHandle(handle))
}
