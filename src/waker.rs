use std::time::Duration;

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
    pub fn request_redraw(&self) {
        self.inner.request_redraw()
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        self.inner.request_redraw_after(duration)
    }

    pub fn request_poll(&self) {
        self.inner.request_poll()
    }
}
