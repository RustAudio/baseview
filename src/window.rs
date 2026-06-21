use crate::context::WindowContext;
use crate::handler::WindowHandler;
use crate::platform;
use crate::window_open_options::WindowOpenOptions;
use dpi::{LogicalSize, PhysicalSize, Pixel};
use raw_window_handle::HasWindowHandle;
use std::marker::PhantomData;

pub struct WindowHandle {
    window_handle: platform::WindowHandle,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl WindowHandle {
    fn new(window_handle: platform::WindowHandle) -> Self {
        Self { window_handle, phantom: PhantomData }
    }

    /// Close the window
    pub fn close(&self) {
        self.window_handle.close();
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    pub fn is_open(&self) -> bool {
        self.window_handle.is_open()
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

#[derive(Debug, Copy, Clone)]
pub struct WindowSize {
    pub physical: PhysicalSize<u32>,
    pub logical: LogicalSize<f64>,
    pub scale_factor: f64,
}

impl WindowSize {
    pub fn from_physical(physical: PhysicalSize<u32>, scale_factor: f64) -> Self {
        Self { physical, logical: physical.to_logical(scale_factor), scale_factor }
    }

    pub fn from_logical(logical: LogicalSize<f64>, scale_factor: f64) -> Self {
        Self { physical: logical.to_physical(scale_factor), logical, scale_factor }
    }
}

impl<P: Pixel> From<WindowSize> for PhysicalSize<P> {
    fn from(size: WindowSize) -> Self {
        size.physical.cast()
    }
}

impl<P: Pixel> From<WindowSize> for LogicalSize<P> {
    fn from(size: WindowSize) -> Self {
        size.logical.cast()
    }
}
