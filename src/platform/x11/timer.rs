use crate::platform::prelude::PlatformResult;
use crate::platform::x11::host_handle::HostHandle;
use baseview_host::host::TimerId;
use slotmap::{DefaultKey, DenseSlotMap, Key};
use std::cell::RefCell;
use std::time::{Duration, Instant};

// Compatible with both TimerHandles from host (u32) and slotmap keys (u64)
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct TimerHandle(u64);

pub enum TimerManager {
    Hosted(HostedTimerStore),
    Standalone(StandaloneTimerStore),
}

impl TimerManager {
    pub fn new(host: &HostHandle) -> Self {
        if host.timer_support().is_some() {
            Self::Hosted(HostedTimerStore::new())
        } else {
            Self::Standalone(StandaloneTimerStore::new())
        }
    }

    pub fn create_timer(
        &self, duration: Duration, host: &HostHandle,
    ) -> PlatformResult<TimerHandle> {
        match self {
            TimerManager::Standalone(store) => Ok(store.insert_new_timer(duration)),
            TimerManager::Hosted(store) => {
                let Some(timer_support) = host.timer_support() else { unreachable!() };
                let id = timer_support.register_timer(duration)?;
                store.insert_new_timer(id);

                Ok(TimerHandle(id.0.into()))
            }
        }
    }

    pub fn tick_next_timer(&self) -> (Option<Instant>, Option<TimerHandle>) {
        match self {
            TimerManager::Hosted(_) => (None, None), // Timers are not manually ticked in hosted mode
            TimerManager::Standalone(store) => store.tick_next_timer(),
        }
    }

    pub fn get_handle_if_exists(&self, id: TimerId) -> Option<TimerHandle> {
        let TimerManager::Hosted(store) = self else { return None };

        if store.exists(id) {
            Some(TimerHandle(id.0.into()))
        } else {
            None
        }
    }

    pub fn destroy_all(&self, host: &HostHandle) {
        match self {
            TimerManager::Standalone(store) => store.destroy_all(),
            TimerManager::Hosted(store) => {
                let Some(timer_support) = host.timer_support() else { unreachable!() };
                for id in store.take_all() {
                    if let Err(e) = timer_support.unregister_timer(id) {
                        crate::warn!("Failed to unregister host timer: {}", e);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct Timer {
    interval: Duration,
    next_trigger: Option<Instant>,
}

pub(crate) struct HostedTimerStore(RefCell<Vec<TimerId>>);

impl HostedTimerStore {
    fn new() -> Self {
        Self(vec![].into())
    }

    fn insert_new_timer(&self, handle: TimerId) {
        self.0.borrow_mut().push(handle)
    }

    fn exists(&self, handle: TimerId) -> bool {
        self.0.borrow().contains(&handle)
    }

    fn take_all(&self) -> Vec<TimerId> {
        std::mem::take(&mut self.0.borrow_mut())
    }
}

pub(crate) struct StandaloneTimerStore(RefCell<DenseSlotMap<DefaultKey, Timer>>);

impl StandaloneTimerStore {
    fn new() -> Self {
        Self(DenseSlotMap::new().into())
    }

    fn insert_new_timer(&self, interval: Duration) -> TimerHandle {
        let mut store = self.0.borrow_mut();
        let entry = Timer { interval, next_trigger: Instant::now().checked_add(interval) };

        let key = store.insert(entry);
        TimerHandle(key.data().as_ffi())
    }

    fn tick_next_timer(&self) -> (Option<Instant>, Option<TimerHandle>) {
        let mut store = self.0.borrow_mut();
        let now = Instant::now();

        let mut soonest_trigger = None;
        let mut triggered_key = None;

        for (key, timer) in store.iter_mut() {
            // Interval was so long it overflowed. This will essentially never trigger, so we just never trigger it.
            let Some(next_trigger) = timer.next_trigger else { continue };

            if triggered_key.is_none() && next_trigger <= now {
                // Timer triggered!
                triggered_key = Some(TimerHandle(key.data().as_ffi()));
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
