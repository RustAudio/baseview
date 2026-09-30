use crate::wrappers::win32::window::{HWnd, PostMessageExt, SyncHwnd};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub struct WindowWakerSource {
    data: Arc<OnceLock<SyncHwnd>>,
}

impl WindowWakerSource {
    pub fn new() -> Self {
        Self { data: Arc::new(OnceLock::new()) }
    }

    pub fn set(&self, hwnd: HWnd) {
        let Ok(()) = self.data.set(hwnd.into()) else { unreachable!() };
    }

    pub fn waker(&self) -> WindowWaker {
        WindowWaker { shared: Arc::clone(&self.data) }
    }
}

#[derive(Clone)]
pub struct WindowWaker {
    shared: Arc<OnceLock<SyncHwnd>>,
}

impl WindowWaker {
    pub fn request_redraw(&self) {
        self.request_redraw_after(Duration::ZERO)
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        let Some(hwnd) = self.shared.get() else { return };
        hwnd.post_request_redraw(duration)
    }

    pub fn request_poll(&self) {
        let Some(hwnd) = self.shared.get() else { return };
        hwnd.post_request_poll()
    }
}
