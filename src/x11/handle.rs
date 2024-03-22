use raw_window_handle::{RawWindowHandle, XcbWindowHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct HandleShared {
    close_requested: AtomicBool,
    is_open: AtomicBool,
}

pub struct UninitializedWindowHandle {
    shared: Arc<HandleShared>,
}

impl UninitializedWindowHandle {
    pub fn window_opened(self, raw_window_handle: XcbWindowHandle) -> WindowHandle {
        WindowHandle { raw_window_handle, shared: self.shared }
    }
}

pub struct WindowHandle {
    raw_window_handle: XcbWindowHandle,
    shared: Arc<HandleShared>,
}

impl WindowHandle {
    pub fn close(&self) {
        // FIXME: This will need to be changed from just setting an atomic to somehow
        // synchronizing with the window being closed (using a synchronous channel, or
        // by joining on the event loop thread).

        self.shared.close_requested.store(true, Ordering::Relaxed);
    }

    pub fn is_open(&self) -> bool {
        self.shared.is_open.load(Ordering::Relaxed)
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        if self.is_open() {
            return self.raw_window_handle.into();
        }

        XcbWindowHandle::empty().into()
    }
}

/// Receives the requests sent from the [`WindowHandle`]
pub struct WindowHandleReceiver {
    shared: Arc<HandleShared>,
}

impl WindowHandleReceiver {
    pub fn new() -> (Self, UninitializedWindowHandle) {
        let shared = Arc::new(HandleShared {
            close_requested: AtomicBool::new(false),
            is_open: AtomicBool::new(true), // This isn't observable until WindowHandle is created
        });

        (Self { shared: shared.clone() }, UninitializedWindowHandle { shared })
    }

    pub fn close_requested(&self) -> bool {
        self.shared.close_requested.load(Ordering::Relaxed)
    }
}

// Notify the external handles that the window has been closed
impl Drop for WindowHandleReceiver {
    fn drop(&mut self) {
        self.shared.is_open.store(false, Ordering::Relaxed);
    }
}
