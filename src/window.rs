use crate::handler::WindowHandlerBuilder;
use crate::platform;
use crate::*;
use dpi::{LogicalSize, PhysicalSize, Pixel};
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

    pub fn run_until_closed(self) -> Result<(), Error> {
        self.window_handle.run_until_closed()?;
        Ok(())
    }

    /// The current size of the window.
    pub fn size(&self) -> WindowSize {
        self.window_handle.size()
    }

    /// Close the window
    pub fn close(self) {
        drop(self)
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    pub fn is_open(&self) -> bool {
        self.window_handle.is_open()
    }
}

pub fn create_window<H: WindowHandler>(
    builder: WindowOpenOptions,
    handler: impl FnOnce(WindowContext) -> Result<H, HandlerError> + Send + 'static,
) -> Result<WindowHandle, Error> {
    Ok(WindowHandle::new(platform::WindowHandle::create_window(
        builder,
        WindowHandlerBuilder::new(handler),
    )?))
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
