use calloop::LoopSignal;
use std::time::Duration;

#[derive(Clone)]
pub struct WindowWaker {
    pub(crate) loop_signal: LoopSignal,
}

impl WindowWaker {
    pub fn request_redraw(&self) {
        self.request_redraw_after(Duration::ZERO)
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        todo!()
    }

    pub fn request_poll(&self) {
        todo!()
    }
}
