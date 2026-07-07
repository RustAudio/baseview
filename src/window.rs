use super::*;
use dpi::{LogicalSize, PhysicalSize, Pixel, Size};
use raw_window_handle::HasWindowHandle;
use std::error::Error;
use std::marker::PhantomData;

pub struct WindowHandle {
    window_handle: platform::WindowHandle,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl WindowHandle {
    pub(crate) fn new(window_handle: platform::WindowHandle) -> Self {
        Self { window_handle, phantom: PhantomData }
    }

    /// Close the window
    pub fn close(self) {
        self.window_handle.destroy();
    }

    pub fn run_until_closed(self) -> Result<(), Box<dyn Error>> {
        todo!()
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    pub fn is_open(&self) -> bool {
        self.window_handle.is_open()
    }

    pub fn suggest_fallback_scale(&self, fallback_scale: Option<f64>) {
        todo!()
    }

    pub fn resize(&self, size: impl Into<Size>) {
        todo!()
    }

    pub fn size(&self) -> WindowSize {
        todo!()
    }

    pub fn make_floating(&self) {
        todo!()
    }

    pub fn set_parent(&self, parent: &impl HasWindowHandle) {
        todo!()
    }

    pub fn show(&self) {
        todo!()
    }

    pub fn hide(&self) {
        todo!()
    }

    pub fn set_title(&self, title: impl Into<String>) {
        todo!()
    }
}

pub struct Window {
    _private: (),
}

impl Window {
    pub fn open_parented<H: WindowHandler>(
        parent: &impl HasWindowHandle, options: WindowOpenOptions,
        build: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) -> WindowHandle {
        let window_handle = platform::Window::open_parented(parent, options, build);
        WindowHandle::new(window_handle)
    }

    pub fn open_blocking<H: WindowHandler>(
        options: WindowOpenOptions, build: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) {
        platform::Window::open_blocking(options, build)
    }
}
