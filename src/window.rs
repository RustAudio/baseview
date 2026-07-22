use crate::handler::WindowHandlerBuilder;
use crate::host::Host;
use crate::platform;
use crate::*;
use dpi::{LogicalSize, PhysicalSize, Pixel, Size};
use std::marker::PhantomData;

/// A handle to a Window created by baseview.
///
/// Unlike some other windowing libraries like `winit`, baseview [`Window`]s manage their own
/// lifecycle.
///
/// All of its events and internal operations (such as rendering) are handled in a separate
/// [`WindowHandler`] type, which is owned by the window itself.
///
/// Dropping this [`Window`] handle will always destroy the window, and drop its associated
/// [`WindowHandler`] and [`Host`] types.
///
/// # Window lifecycle and ownership
///
/// Owning this [`Window`] does not mean you fully own the window itself, per se.
/// While you control when the window is [`create`d](Self::create), you do not have the sole control
/// over when it is destroyed.
///
/// The lifetime of this [`Window`] handle is going to be the longest possible lifetime for the
/// underlying platform window, but it can be destroyed earlier than this.
///
/// This is because while dropping this handle will always destroy the window, it can be destroyed
/// from other factors, such as:
///
/// * The [`WindowHandler`] decided to close the window itself, e.g. by calling [`WindowContext::request_close`] from the user clicking an internal "close" button;
/// * The [`WindowContext`] encountered a fatal error (e.g. during rendering) or panicked,
///   and cannot operate anymore.
/// * The underlying platform closed or destroyed the window directly.
/// * The connection to the display server (on e.g. X11) was lost.
///
/// This type makes enables to handle those cases safely: most methods will either return errors or
/// become no-ops. You can use the [`Window::is_open`] method to know if the window has been closed.
///
pub struct Window {
    inner: platform::WindowHandle,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl Window {
    /// Creates a new window, using the given [`WindowOpenOptions`] and a builder closure to
    /// create the associated [`WindowHandler`].
    ///
    /// This function creates the window but does not open or show it.
    /// You must use the [`show`](Self::show) method to actually open it.
    /// (unless you use [`run_until_closed`](Self::run_until_closed), which does it automatically)
    #[inline]
    pub fn create<H: WindowHandler>(
        options: WindowOpenOptions,
        handler: impl FnOnce(WindowContext) -> Result<H, HandlerError> + Send + 'static,
    ) -> Result<Window, Error> {
        Self::create_with_host(options, handler, None)
    }

    /// Creates a new window, using the given [`WindowOpenOptions`] and a builder closure to
    /// create the associated [`WindowHandler`], as well as an optional [`Host`] containing callbacks
    /// to a potential system that is hosting the window (e.g. in a plug-in setting).
    ///
    /// This function creates the window but does not open or show it.
    /// You must use the [`show`](Self::show) method to actually open it.
    /// (unless you use [`run_until_closed`](Self::run_until_closed), which does it automatically)
    ///
    /// Calling this function with [`None`] for the `host` value is equivalent to calling
    /// [`create`](Self::create).
    pub fn create_with_host<H: WindowHandler>(
        options: WindowOpenOptions,
        handler: impl FnOnce(WindowContext) -> Result<H, HandlerError> + Send + 'static,
        host: impl Into<Option<Host>>,
    ) -> Result<Window, Error> {
        Ok(Self {
            inner: platform::WindowHandle::create_window(
                options,
                WindowHandlerBuilder::new(handler),
                host.into().unwrap_or_else(Host::default),
            )?,
            phantom: PhantomData,
        })
    }

    /// Blocks the thread and runs an event loop until the window is closed.
    ///
    /// The window is shown automatically if it wasn't already.
    #[inline]
    pub fn run_until_closed(self) -> Result<(), Error> {
        self.inner.run_until_closed()?;
        Ok(())
    }

    /// The current size of the window.
    #[inline]
    pub fn size(&self) -> WindowSize {
        self.inner.size()
    }

    /// Resizes the window to the given [`Size`].
    ///
    /// The `size` can be provided in either physical or logical pixels.
    ///
    /// Using this method does *not* trigger the [`HostCallbacks::request_resize`](host::HostCallbacks) callback.
    #[inline]
    pub fn resize(&self, size: Size) -> Result<(), Error> {
        self.inner.resize(size)?;
        Ok(())
    }

    /// Suggests a fallback scale factor, if Baseview couldn't get one from the platform.
    ///
    /// If the platform does already provide an accurate scaling factor, this doesn't do anything.
    ///
    /// If the given fallback scale factor is actually useful and different from the current one
    /// (1.0 by default), this will resize and redraw the window accordingly.
    ///
    /// # Platform compatibility notes.
    ///
    /// On Win32, this value is used if running on early versions of Windows 10 (or earlier).
    ///
    /// On X11, this value is used if no `Xft.dpi`setting is set.
    ///
    /// On macOS, this function is always a no-op.
    #[inline]
    pub fn suggest_fallback_scale_factor(&self, scale_factor: f64) -> Result<(), Error> {
        self.inner.suggest_scale_factor(scale_factor)?;
        Ok(())
    }

    /// Closes and destroys the window.
    ///
    /// This releases all resources the window uses.
    ///
    /// It is guaranteed that no other objects (e.g. the parent window) are used by this window after
    /// this call.
    ///
    /// Calling this method is more explicit, but otherwise identical to just dropping this [`Window`].
    #[inline]
    pub fn close(self) {
        drop(self)
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    #[inline]
    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }

    /// Performs the work the window thread had scheduled for the main thread.
    ///
    /// This must be called back on the main thread, as a response to [`HostMainThreadCaller::call_main_thread`](host::HostMainThreadCaller::call_main_thread).
    ///
    /// # Platform compatibility notes
    ///
    /// Only the X11 platform has a separate window thread, so this is only needed to run host callbacks on X11.
    ///
    /// On Windows and macOS, this is always a no-op.
    #[inline]
    pub fn host_main_thread_callback(&mut self) {
        self.inner.handle_main_thread_callback()
    }

    /// Reparents this window using the given `parent`.
    ///
    /// If the window was a floating window, it will become parented.
    #[inline]
    pub fn set_parent(&self, parent: impl Into<ParentWindowHandle>) -> Result<(), Error> {
        self.inner.set_parent(parent.into().inner)?;
        Ok(())
    }

    /// Shows the window to the screen.
    #[inline]
    pub fn show(&self) -> Result<(), Error> {
        self.inner.show()?;
        Ok(())
    }

    /// Hides the window from the screen.
    ///
    /// The window will still exist, and it might still receive some events, but rendering will be
    /// paused and the user will not be able to see or interact with it.
    #[inline]
    pub fn hide(&self) -> Result<(), Error> {
        self.inner.hide()?;
        Ok(())
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
