use crate::platform::x11::window_thread::WindowThreadShared;
use calloop::LoopSignal;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct WindowWaker {
    pub(crate) loop_signal: LoopSignal,
    pub(crate) shared: Arc<WindowThreadShared>,
}

impl WindowWaker {
    pub fn request_redraw(&self) {
        self.request_redraw_after(Duration::ZERO)
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        self.shared.request_redraw_after(duration);
        self.loop_signal.wakeup();
    }

    pub fn request_poll(&self) {
        self.shared.request_poll();
        self.loop_signal.wakeup();
    }
}
