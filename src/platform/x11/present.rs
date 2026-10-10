use crate::platform::prelude::*;
use crate::platform::x11::handler::Handler;
use crate::platform::x11::host_handle::HostHandle;
use crate::platform::x11::sizing::SizingState;
use crate::platform::x11::window_shared::WindowShared;
use crate::DamageArea;
use calloop::timer::TimeoutAction;
use std::cell::Cell;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::present::{CompleteKind, CompleteNotifyEvent};
use x11rb::protocol::xproto::ExposeEvent;

pub struct PresentStateShared {
    present_notify_requested: Cell<bool>,
    poll_requested: Cell<bool>,
}

impl PresentStateShared {
    pub fn new() -> Self {
        Self { present_notify_requested: false.into(), poll_requested: false.into() }
    }

    pub fn request_present_notify(&self) {
        self.present_notify_requested.set(true)
    }

    pub fn request_present_notify_after(
        &self, duration: Duration, host: &HostHandle, redraw_timers: &TimerManager,
    ) {
        if duration.is_zero() || duration.as_millis() < 1 {
            self.request_present_notify();
            return;
        }

        if let Err(e) = redraw_timers.create_timer(duration, host) {
            warn!("{}", e);
            self.request_present_notify();
        }
    }
}

pub struct PresentState {
    draw_now: Cell<bool>,
    last_requested_serial: Cell<Option<u32>>,
    last_received_present: Cell<Option<(u32, u64)>>,

    fallback_frame_timer_manager: TimerManager,
    fallback_frame_timer: Cell<Option<TimerHandle>>,
}

impl PresentState {
    pub fn new(host: &HostHandle) -> Self {
        Self {
            draw_now: false.into(),
            last_requested_serial: None.into(),
            last_received_present: None.into(),
            fallback_frame_timer_manager: TimerManager::new(host),
            fallback_frame_timer: None.into(),
        }
    }

    pub fn handle_requests(
        &self, shared: &WindowShared, handler: &Handler, host: &HostHandle,
    ) -> Result<(), FatalError> {
        let shared_state = &shared.present_state;

        if self.draw_now.get() {
            handler.poll();
            shared.present_state.present_notify_requested.set(false);

            handler.draw()?;

            shared_state.poll_requested.set(false);
            self.draw_now.set(false);

            shared.connection.conn.flush()?;
        } else if shared_state.poll_requested.take() {
            handler.poll();
        }

        self.handle_present_notify(&shared.present_state, &shared.xcb_window, host)?;

        Ok(())
    }

    pub fn handle_expose_event(
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

    pub fn handle_window_mapped(
        &self, shared: &PresentStateShared, window: &XcbWindow, host: &HostHandle,
    ) -> Result<(), FatalError> {
        if window.present_supported() && window.present_select_input()? {
            shared.present_notify_requested.set(true);
        } else {
            self.setup_fallback_frame_timer(host)?;
        }

        Ok(())
    }

    pub fn handle_present_complete_notify(&self, e: CompleteNotifyEvent) {
        if e.kind != CompleteKind::NOTIFY_MSC {
            return;
        }

        let Some(last_requested_serial) = self.last_requested_serial.get() else { return };

        if last_requested_serial != e.serial {
            return;
        }

        if let Some((last_received_serial, last_received_msc)) = self.last_received_present.get() {
            if last_received_serial == e.serial {
                return;
            }

            if e.msc <= last_received_msc {
                self.last_received_present.set(Some((e.serial, e.msc)));
                return;
            }
        }

        self.last_received_present.set(Some((e.serial, e.msc)));
        self.draw_now.set(true);
    }

    fn tick_fallback_frame_timer(&self) -> Option<Instant> {
        let Some(timer_handle) = self.fallback_frame_timer.get() else { return None };

        let (deadline, triggered) = self.fallback_frame_timer_manager.tick_next_timer();
        if let Some(triggered) = triggered {
            if triggered == timer_handle {
                self.draw_now.set(true);
            }
        }

        deadline
    }

    fn setup_fallback_frame_timer(&self, host: &HostHandle) -> Result<(), PlatformError> {
        const FRAME_INTERVAL: Duration = Duration::from_millis(15);

        if let Some(previous) = self.fallback_frame_timer.take() {
            self.fallback_frame_timer_manager.destroy_timer(previous, host);
        }

        self.fallback_frame_timer_manager.create_timer(FRAME_INTERVAL, host)?;

        Ok(())
    }

    pub fn handle_present_notify(
        &self, shared: &PresentStateShared, window: &XcbWindow, host: &HostHandle,
    ) -> Result<(), PlatformError> {
        if !shared.present_notify_requested.get() {
            return Ok(());
        }

        if !window.present_supported() {
            shared.present_notify_requested.set(false);
            return Ok(());
        }

        let (next_serial, target_msc) =
            match (self.last_requested_serial.get(), self.last_received_present.get()) {
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
            self.last_requested_serial.set(Some(next_serial));
        } else {
            self.last_requested_serial.set(None);
            self.setup_fallback_frame_timer(host)?;
        }
        shared.present_notify_requested.set(false);

        Ok(())
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
