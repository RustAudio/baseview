use crate::platform::prelude::PlatformResult;
use crate::platform::x11::host_handle::HostHandle;
use slotmap::{DefaultKey, DenseSlotMap, Key};
use std::cell::RefCell;
use std::time::{Duration, Instant};

// Compatible with both TimerHandles from host (u32) and
pub type TimerHandle = u64;

pub enum TimerManager {
    Hosted(),
    Standalone(StandaloneTimerStore),
}

impl TimerManager {
    pub fn new(host: &HostHandle) -> Self {
        if host.timer_support().is_some() {
            todo!()
        } else {
            Self::Standalone(StandaloneTimerStore::new())
        }
    }

    pub fn create_timer(
        &self, duration: Duration, host: &HostHandle,
    ) -> PlatformResult<TimerHandle> {
        match self {
            TimerManager::Hosted() => todo!(),
            TimerManager::Standalone(store) => Ok(store.insert_new_timer(duration).data().as_ffi()),
        }
    }

    pub fn tick_next_timer(&self) -> (Option<Instant>, Option<TimerHandle>) {
        match self {
            TimerManager::Hosted() => todo!(),
            TimerManager::Standalone(store) => store.tick_next_timer(),
        }
    }

    pub fn destroy_all(&self) {
        match self {
            TimerManager::Hosted() => todo!(),
            TimerManager::Standalone(store) => store.destroy_all(),
        }
    }
}

#[derive(Debug)]
struct Timer {
    interval: Duration,
    next_trigger: Option<Instant>,
}

struct StandaloneTimerStore(RefCell<DenseSlotMap<DefaultKey, Timer>>);

impl StandaloneTimerStore {
    pub fn new() -> Self {
        Self(DenseSlotMap::new().into())
    }

    pub fn insert_new_timer(&self, interval: Duration) -> DefaultKey {
        let mut store = self.0.borrow_mut();
        store.insert(Timer { interval, next_trigger: Instant::now().checked_add(interval) })
    }

    pub fn tick_next_timer(&self) -> (Option<Instant>, Option<TimerHandle>) {
        let mut store = self.0.borrow_mut();
        let now = Instant::now();

        let mut soonest_trigger = None;
        let mut triggered_key = None;

        for (key, timer) in store.iter_mut() {
            // Interval was so long it overflowed. This will essentially never trigger, so we just never trigger it.
            let Some(next_trigger) = timer.next_trigger else { continue };

            if triggered_key.is_none() && next_trigger <= now {
                // Timer triggered!
                triggered_key = Some(key.data().as_ffi());
                timer.next_trigger = now.checked_add(timer.interval);
            }

            let Some(next_trigger) = timer.next_trigger else { continue };

            match soonest_trigger {
                Some(previous_soonest_trigger) => {
                    if previous_soonest_trigger > next_trigger {
                        soonest_trigger = Some(next_trigger);
                    }
                }
                None => soonest_trigger = Some(next_trigger),
            }
        }

        (soonest_trigger, triggered_key)
    }

    fn destroy_all(&self) {
        self.0.borrow_mut().clear();
    }
}
