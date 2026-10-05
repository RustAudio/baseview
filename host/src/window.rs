use crate::dpi::{NativeSize, WindowSize};
use dpi::Size;
use raw_window_handle::HasWindowHandle;
use std::error::Error;

/// A handle to a hostable Window.
///
/// Unlike some other windowing libraries like `winit`, [`HostedWindow`]s manage their own
/// lifecycle.
///
/// All of its events and internal operations (such as rendering) are handled in a separate
/// type, which is owned by the window itself.
///
/// Dropping this [`HostedWindow`] handle will always destroy the window.
///
/// # Window lifecycle and ownership
///
/// Owning this [`HostedWindow`] does not mean you fully own the window itself, per se.
/// While you may have control over when the window is created, you do not have the sole control
/// over when it is destroyed.
///
/// The lifetime of this [`HostedWindow`] handle is going to be the longest possible lifetime for the
/// underlying platform window, but it can be destroyed earlier than this.
///
/// This is because while dropping this handle will always destroy the window, it can be destroyed
/// from other factors, such as:
///
/// * The implementation decided to close the window itself, e.g. from the user clicking an internal "close" button;
/// * The implementation encountered a fatal error (e.g. during rendering) or panicked, and cannot operate anymore.
/// * The underlying platform closed or destroyed the window directly.
/// * The connection to the display server (on e.g. X11) was lost.
///
/// This type makes enables to handle those cases safely: most methods will either return errors or
/// become no-ops. You can use the [`HostedWindow::is_open`] method to know if the window has been closed.
///
pub trait HostedWindow {
    /// Blocks the thread and runs an event loop until the window is closed.
    ///
    /// The window is shown automatically if it wasn't already.
    fn run_until_closed(self) -> Result<(), Box<dyn Error>>
    where
        Self: Sized;

    /// The current size of the window.
    fn size(&self) -> WindowSize;

    /// Resizes the window to the given [`Size`].
    ///
    /// The `size` can be provided in either physical or logical pixels.
    ///
    /// Using this method does *not* trigger the [`HostCallbacks::request_resize`](host::HostCallbacks) callback.
    fn resize(&self, size: Size) -> Result<(), Box<dyn Error>>;

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
    fn suggest_fallback_scale_factor(&self, scale_factor: f64) -> Result<(), Box<dyn Error>>;

    /// Closes and destroys the window.
    ///
    /// This releases all resources the window uses.
    ///
    /// It is guaranteed that no other objects (e.g. the parent window) are used by this window after
    /// this call.
    ///
    /// Calling this method is more explicit, but otherwise identical to just dropping this [`Window`].
    fn close(self)
    where
        Self: Sized,
    {
        drop(self)
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    fn is_open(&self) -> bool;

    /// Returns `true` if the window can be resized by the user, `false` otherwise.
    ///
    /// This is set by the [`WindowSettings::resizable`] field.
    fn is_resizable(&self) -> bool;

    /// Returns the minimum size of the window, if it has one.
    ///
    /// This is set by the [`WindowSettings::min_size`] field.
    fn min_size(&self) -> Option<Size>;

    /// Returns the minimum size of the window, if it has one.
    ///
    /// This is set by the [`WindowSettings::max_size`] field.
    fn max_size(&self) -> Option<Size>;

    /// Performs the work the window thread had scheduled for the main thread.
    ///
    /// This must be called back on the main thread, as a response to [`HostMainThreadCaller::call_main_thread`](host::HostMainThreadCaller::call_main_thread).
    ///
    /// # Platform compatibility notes
    ///
    /// Only the X11 platform has a separate window thread, so this is only needed to run host callbacks on X11.
    ///
    /// On Windows and macOS, this is always a no-op.
    fn host_main_thread_callback(&self);

    /// Reparents this window using the given `parent`.
    ///
    /// # Panics
    ///
    /// This function can panic if the window did not have a parent, but was already created as a floating window.
    ///
    /// It can also panic if the given `parent` is invalid for the current platform.
    fn set_parent(&self, parent: &dyn HasWindowHandle) -> Result<(), Box<dyn Error>>;

    /// Shows the window to the screen.
    fn show(&self) -> Result<(), Box<dyn Error>>;

    /// Hides the window from the screen.
    ///
    /// The window will still exist, and it might still receive some events, but rendering will be
    /// paused and the user will not be able to see or interact with it.
    fn hide(&self) -> Result<(), Box<dyn Error>>;

    /// Adjusts the given size to the window's size constraints.
    fn adjust_size(&self, size: NativeSize<u32>) -> NativeSize<u32>;

    fn request_poll(&self) -> Result<(), Box<dyn Error>>;
}
