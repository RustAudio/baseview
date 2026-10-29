use crate::platform::win::window_state::RedrawTimer;
use crate::platform::BaseviewWindow;
use crate::wrappers::win32::window::{HWnd, WmDraw};
use crate::wrappers::win32::{TimerId, TimerType};
use crate::DamageArea;

impl WmDraw for BaseviewWindow {
    fn on_timer(&self, window: HWnd, timer_id: TimerId) -> bool {
        if timer_id != RedrawTimer::ID {
            return false;
        }

        if !self.window_state.redraw_timer.is_running() {
            return false;
        };

        window.invalidate_window();
        true
    }

    fn on_paint(&self, window: HWnd) {
        if let Some(rect) = window.get_update_rect() {
            self.notify_damage(DamageArea::Rect(rect.into()));
            self.handle_draw();
            window.validate_rect(rect);
        }
    }

    fn on_redraw_requested(&self) {
        self.window_state.request_redraw();
    }

    fn on_poll_requested(&self) {
        self.handle_poll();
    }
}
