use crate::context::WindowContext;
use crate::handler::WindowHandler;
use crate::platform;
use crate::window_open_options::WindowOpenOptions;
use dpi::{LogicalSize, PhysicalSize, Pixel, Size};
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

    pub fn set_parent(&self, parent: impl HasWindowHandle + 'static) {
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

/// A window's size, which can be read in either logical or physical pixels.
///
/// Methods that produce this type in baseview guarantee that either the physical or the logical
/// size is directly from the underlying platform API.
///
/// This means that for either of the size types, there is at most only one conversion performed,
/// which minimizes errors that may occur due to rounding.
#[derive(Debug, Copy, Clone)]
pub struct WindowSize {
    /// The window's size in physical pixels
    pub physical: PhysicalSize<u32>,
    /// The window's size in logical pixels
    pub logical: LogicalSize<f64>,
    /// The backing scale factor of the window.
    ///
    /// This is the value used to convert between the physical and logical sizes.
    pub scale_factor: f64,
}

impl WindowSize {
    /// Constructs a [`WindowSize`] from a given [`PhysicalSize`] and `scale_factor`.
    ///
    /// The [`LogicalSize`] is converted from the given physical size, using the given scale factor.
    #[inline]
    pub fn from_physical(physical: PhysicalSize<u32>, scale_factor: f64) -> Self {
        Self { physical, logical: physical.to_logical(scale_factor), scale_factor }
    }

    /// Constructs a [`WindowSize`] from a given [`LogicalSize`] and `scale_factor`.
    ///
    /// The [`PhysicalSize`] is converted from the given physical size, using the given scale factor.
    #[inline]
    pub fn from_logical(logical: LogicalSize<f64>, scale_factor: f64) -> Self {
        Self { physical: logical.to_physical(scale_factor), logical, scale_factor }
    }
}

impl<P: Pixel> From<WindowSize> for PhysicalSize<P> {
    #[inline]
    fn from(size: WindowSize) -> Self {
        size.physical.cast()
    }
}

impl<P: Pixel> From<WindowSize> for LogicalSize<P> {
    #[inline]
    fn from(size: WindowSize) -> Self {
        size.logical.cast()
    }
}
