use super::*;
use crate::handler::WindowHandlerBuilder;
use dpi::Size;
use raw_window_handle::HasWindowHandle;
use std::error::Error;
use std::marker::PhantomData;

pub struct Window {
    inner: platform::Window,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl Window {
    pub(crate) fn new(inner: platform::Window) -> Self {
        Self { inner, phantom: PhantomData }
    }

    /// Close the window
    #[inline]
    pub fn close(self) {
        drop(self);
    }

    #[inline]
    pub fn run_until_closed(self) -> Result<(), Box<dyn Error>> {
        self.inner.run_until_closed()
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    #[inline]
    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }

    /*
    pub fn suggest_fallback_scale(&self, fallback_scale: Option<f64>) {
        todo!()
    }

    #[inline]
    pub fn resize(&self, size: impl Into<Size>) {
        todo!()
    }

    #[inline]
    pub fn size(&self) -> WindowSize {
        todo!()
    }

    #[inline]
    pub fn make_floating(&self) {
        todo!()
    }

    #[inline]
    pub fn set_parent(&self, parent: &impl HasWindowHandle) {
        todo!()
    }

    #[inline]
    pub fn show(&self) {
        todo!()
    }

    #[inline]
    pub fn hide(&self) {
        todo!()
    }

    #[inline]
    pub fn set_title(&self, title: impl Into<String>) {
        todo!()
    }*/
}

pub fn create_window<H: WindowHandler>(
    builder: WindowBuilder, handler: impl FnOnce(WindowContext) -> H + Send + 'static,
) -> Window {
    Window::new(platform::Window::create_window(builder, WindowHandlerBuilder::new(handler)))
}
