use crate::platform::macos::view::BaseviewView;
use crate::wrappers::appkit::View;
use block2::RcBlock;
use objc2::__framework_prelude::Retained;
use objc2::rc::Weak;
use objc2_foundation::NSTimer;
use std::cell::RefCell;
use std::ptr::NonNull;
use std::time::Duration;

#[derive(Clone, Eq)]
pub struct TimerHandle {
    timer: NonNull<NSTimer>,
}

impl PartialEq for TimerHandle {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.timer == other.timer
    }
}

pub struct TimerManager {
    timers: RefCell<Vec<Retained<NSTimer>>>,
}

impl TimerManager {
    pub fn new() -> Self {
        TimerManager { timers: vec![].into() }
    }

    pub fn create_timer(&self, view: Weak<View<BaseviewView>>, duration: Duration) -> TimerHandle {
        let timer = create_timer(view, duration);

        let Some(ptr) = NonNull::new(Retained::as_ptr(&timer).cast_mut()) else { unreachable!() };

        let handle = TimerHandle { timer: ptr };

        self.add_timer(timer);

        handle
    }

    fn add_timer(&self, timer: Retained<NSTimer>) {
        self.timers.borrow_mut().push(timer);
    }
}

impl Drop for TimerManager {
    fn drop(&mut self) {
        for timer in self.timers.take() {
            timer.invalidate();
        }
    }
}

fn create_timer(view: Weak<View<BaseviewView>>, duration: Duration) -> Retained<NSTimer> {
    let interval = duration.as_secs_f64();
    let block = RcBlock::new(move |t| {
        let Some(view) = view.load() else { return };
        let Some(view) = view.inner_ref() else { return };

        let handle = TimerHandle { timer: t };

        BaseviewView::trigger_timer(view, &handle);
    });

    // SAFETY: block does not need to be sendable
    unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, true, &block) }
}
