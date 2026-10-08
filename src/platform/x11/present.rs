use crate::platform::prelude::*;
use crate::platform::x11::handler::Handler;
use crate::platform::x11::sizing::SizingState;
use crate::platform::x11::window_shared::WindowShared;
use crate::platform::x11::window_thread::RedrawRequested;
use crate::DamageArea;
use calloop::timer::{TimeoutAction, Timer};
use calloop::LoopHandle;
use dpi::{PhysicalPosition, PhysicalSize};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::warn;
use x11rb::connection::Connection;
use x11rb::protocol::present::{CompleteKind, CompleteNotifyEvent};
use x11rb::protocol::xproto::ExposeEvent;

pub struct PresentStateShared {
    present_notify_requested: Cell<bool>,
    poll_requested: Cell<bool>,
}

impl PresentStateShared {
    pub(crate) fn request_present_notify_after(
        &self, duration: Duration, loop_handle: &LoopHandle<EventLoop>,
    ) {
        if duration.is_zero() || duration.as_millis() < 1 {
            self.request_present_notify();
            return;
        }

        let result = loop_handle.insert_source(Timer::from_duration(duration), |_, _, e| {
            e.shared().present_state.request_present_notify();
            TimeoutAction::Drop
        });

        if let Err(e) = result {
            warn!("{}", e);
            self.request_present_notify();
        }
    }
}

impl PresentStateShared {
    pub fn new() -> Self {
        Self { present_notify_requested: false.into(), poll_requested: false.into() }
    }

    pub fn request_present_notify(&self) {
        self.present_notify_requested.set(true)
    }
}

pub struct PresentState {
    draw_now: bool,
    last_requested_serial: Option<u32>,
    last_received_present: Option<(u32, u64)>,
}

impl PresentState {
    pub(crate) fn handle_requests(
        &mut self, shared: &WindowShared, handler: &Handler, loop_handle: &LoopHandle<EventLoop>,
    ) -> Result<(), FatalError> {
        let shared_state = &shared.present_state;
        let thread_state = &shared.main_thread_shared.present;

        // Consume all requests from above poll
        if let Some(redraw_after) = thread_state.take_redraw_request() {
            shared.request_redraw_after(redraw_after)
        }

        if self.draw_now {
            let _ = thread_state.take_poll_request();

            handler.poll();
            shared.present_state.present_notify_requested.set(false);

            handler.draw()?;

            shared_state.poll_requested.set(false);
            self.draw_now = false;

            shared.connection.conn.flush()?;
        } else if thread_state.take_poll_request() || shared_state.poll_requested.take() {
            handler.poll();
        }

        self.handle_present_notify(&shared.present_state, &shared.xcb_window, loop_handle)?;

        Ok(())
    }
}

impl PresentState {
    pub(crate) fn handle_expose_event(
        &self, e: ExposeEvent, handler: &Handler, shared: &WindowShared, sizing_state: &SizingState,
    ) {
        if e.count == 0 {
            shared.present_state.present_notify_requested.set(true);
        }

        let current_window_size = sizing_state.non_coalesced_current_size(shared);

        let damage_rect = DamageRect::new(&e);
        let area = if damage_rect.fully_covers(current_window_size) {
            DamageArea::FullWindow
        } else {
            DamageArea::Rect(damage_rect.into())
        };

        handler.damage(area);
    }
}

impl PresentState {
    pub fn handle_window_mapped(
        &self, shared: &PresentStateShared, window: &XcbWindow, loop_handle: &LoopHandle<EventLoop>,
    ) -> Result<(), FatalError> {
        if window.present_supported() && window.present_select_input()? {
            shared.present_notify_requested.set(true);
        } else {
            Self::setup_fallback_frame_timer(loop_handle)?;
        }

        Ok(())
    }
}

impl PresentState {
    pub fn new() -> Self {
        Self { draw_now: false, last_requested_serial: None, last_received_present: None }
    }

    pub fn handle_present_complete_notify(&mut self, e: CompleteNotifyEvent) {
        if e.kind != CompleteKind::NOTIFY_MSC {
            return;
        }

        let Some(last_requested_serial) = self.last_requested_serial else { return };

        if last_requested_serial != e.serial {
            return;
        }

        if let Some((last_received_serial, last_received_msc)) = self.last_received_present {
            if last_received_serial == e.serial {
                return;
            }

            if e.msc <= last_received_msc {
                self.last_received_present = Some((e.serial, e.msc));
                return;
            }
        }

        self.last_received_present = Some((e.serial, e.msc));
        self.draw_now = true;
    }

    fn setup_fallback_frame_timer(
        loop_handle: &LoopHandle<EventLoop>,
    ) -> Result<(), calloop::Error> {
        const FRAME_INTERVAL: Duration = Duration::from_millis(15);

        fn handle_frame(evloop: &mut EventLoop, previous_deadline: Instant) -> TimeoutAction {
            evloop.present_state.draw_now = true;

            // We'll try to keep a consistent frame pace. If the last frame couldn't be processed in
            // the expected frame time, this will throttle down to prevent multiple frames from
            // being queued up.

            let now = Instant::now();

            let Some(next_deadline) = previous_deadline.checked_add(FRAME_INTERVAL) else {
                return TimeoutAction::ToDuration(FRAME_INTERVAL);
            };

            if next_deadline >= now {
                return TimeoutAction::ToDuration(FRAME_INTERVAL);
            }

            TimeoutAction::ToInstant(next_deadline)
        }

        loop_handle
            .insert_source(Timer::from_duration(FRAME_INTERVAL), |i, _, e| handle_frame(e, i))
            .map_err(|e| e.error)?;

        Ok(())
    }

    pub fn handle_present_notify(
        &mut self, shared: &PresentStateShared, window: &XcbWindow,
        loop_handle: &LoopHandle<EventLoop>,
    ) -> Result<(), FatalError> {
        if !shared.present_notify_requested.get() {
            return Ok(());
        }

        if !window.present_supported() {
            shared.present_notify_requested.set(false);
            return Ok(());
        }

        let (next_serial, target_msc) =
            match (self.last_requested_serial, self.last_received_present) {
                // First request, always send
                (None, None) => (0, 0),
                (Some(sent_serial), Some((received_serial, last_msc)))
                    if sent_serial == received_serial =>
                {
                    (sent_serial.wrapping_add(1), last_msc.wrapping_add(2))
                }
                // We sent our first request but have not gotten a response yet.
                // Or, we sent a request, but the last response we've gotten isn't that one.
                // Do not send.
                _ => {
                    shared.present_notify_requested.set(false);
                    return Ok(());
                }
            };

        if window.present_notify(target_msc, next_serial)?.check_is_ok() {
            self.last_requested_serial = Some(next_serial);
        } else {
            self.last_requested_serial = None;
            Self::setup_fallback_frame_timer(loop_handle)?;
        }
        shared.present_notify_requested.set(false);

        Ok(())
    }
}

pub struct PresentThreadShared {
    poll_requested: AtomicBool,
    redraw_requested_after: Mutex<Option<RedrawRequested>>,
}

impl PresentThreadShared {
    pub(crate) fn new() -> Self {
        Self { poll_requested: false.into(), redraw_requested_after: None.into() }
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        // Ignore a poisoned mutex, we just fully override this value anyway.
        let mut guard = self.redraw_requested_after.lock().unwrap_or_else(|g| g.into_inner());
        *guard = Some(RedrawRequested::from_duration(duration));
    }

    fn take_redraw_request(&self) -> Option<Duration> {
        let mut guard = self.redraw_requested_after.lock().unwrap_or_else(|g| g.into_inner());
        guard.take().map(|w| w.to_duration())
    }

    pub fn request_poll(&self) {
        self.poll_requested.store(true, Ordering::Relaxed);
    }

    fn take_poll_request(&self) -> bool {
        self.poll_requested.swap(false, Ordering::Relaxed)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DamageRect {
    pos: PhysicalPosition<u16>,
    size: PhysicalSize<u16>,
}

impl DamageRect {
    #[inline]
    pub fn new(event: &ExposeEvent) -> Self {
        Self {
            pos: PhysicalPosition::new(event.x, event.y),
            size: PhysicalSize::new(event.width, event.height),
        }
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<u32> {
        PhysicalPosition { x: self.pos.x.into(), y: self.pos.y.into() }
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize { height: self.size.height.into(), width: self.size.width.into() }
    }

    pub fn fully_covers(&self, window_size: PhysicalSize<u16>) -> bool {
        self.pos.x == 0
            && self.pos.y == 0
            && window_size.width <= self.size.width
            && window_size.height <= self.size.height
    }
}
