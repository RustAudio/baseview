use crate::platform::prelude::PlatformResult;
use crate::platform::x11::host_handle::HostHandle;
use calloop::LoopSignal;
use std::task::Waker;
use std::time::Duration;

pub(crate) struct WakerSource {}

impl WakerSource {
    pub fn new(host: &HostHandle) -> Self {
        todo!()
    }

    pub fn new_waker(&self) -> Waker {
        todo!()
    }
}

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
