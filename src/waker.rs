use std::time::Duration;

/// A simple, weak handle to the [`Window`](crate::Window), which can be used to "wake" it from
/// external events or threads.
///
/// It can be cheaply [cloned](Clone) and [send](Send) to be used from other threads with minimal
/// overhead.
#[derive(Clone)]
pub struct WindowWaker {
    pub(crate) inner: crate::platform::WindowWaker,
}

// Assert that PlatformHandle implements both Send & Sync on all platforms
const _: () = {
    const fn assert_impl_all<T: Send + Sync>() {}
    let _: fn() = assert_impl_all::<WindowWaker>;
};

impl WindowWaker {
    /// Schedules a new frame to be drawn.
    pub fn request_redraw(&self) {
        self.inner.request_redraw()
    }

    /// Schedules a new frame to be drawn after the given `duration`.
    pub fn request_redraw_after(&self, duration: Duration) {
        self.inner.request_redraw_after(duration)
    }

    /// Schedules a call to the [`WindowHandler::poll`](crate::WindowHandler::poll) callback, at
    /// the platform's earliest convenience.
    pub fn request_poll(&self) {
        self.inner.request_poll()
    }
}
