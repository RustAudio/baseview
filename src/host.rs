use crate::{HandlerError, WindowSize};
use std::cell::RefCell;

/// A special handler for the Window thread to wake up and call methods on the main thread.
///
/// [`WindowHandle::host_main_thread_callback`](crate::WindowHandle::host_main_thread_callback)
/// should be called as a response to this.
///
/// # Platform compatibility notes
///
/// This is only needed on X11, as Windows and macOS windows already run on the main thread.
pub trait HostMainThreadCaller: Send + 'static {
    /// Schedules a callback on the main thread.
    ///
    /// [`WindowHandle::host_main_thread_callback`](crate::WindowHandle::host_main_thread_callback)
    /// should be called as a response to this.
    ///
    /// # Platform compatibility notes
    ///
    /// Only X11 needs this. This can be implemented as a no-op on Windows and macOS.
    fn call_main_thread(&mut self);
}

/// A handler for baseview windows to interact with their host.
pub trait HostCallbacks: 'static {
    /// Requests the parent window to be resized to accommodate the child window with the given new
    /// size.
    ///
    /// # Errors
    ///
    /// This can return any type of error, indicating the host either failed or denied to handle the
    /// resize request.
    /// If it does, the error is logged and the resize operation is canceled or reverted.
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), HandlerError>;
    /// Notifies the host that the child window has been destroyed for a reason outside the host's
    /// control.
    ///
    /// This can be because the display connection was lost, because the window handler crashed, or
    /// because the window handler decided to close the window itself.
    ///
    /// The host should close its parent window, as it will not show anything useful anymore.
    fn destroyed(&mut self);
}

/// Configuration and callbacks for a window's host.
///
/// # Safety
///
/// This type and its methods are always safe to use.
///
/// It also brings the additional safety guarantee that all handlers given to this types will be
/// destroyed alongside with the window, guaranteeing callbacks cannot be fired after the
/// [`WindowHandle`](crate::WindowHandle) is dropped. (or after this [`Host`] object is dropped, if
/// it never made it to a [`create_window_with_host`](crate::create_window_with_host) call).
pub struct Host {
    #[cfg(target_os = "linux")]
    pub(crate) main_thread: Option<Box<dyn HostMainThreadCaller>>,
    pub(crate) callbacks: Option<RefCell<Box<dyn HostCallbacks>>>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    /// Creates a new, empty host with no callbacks.
    ///
    /// Calling [`create_window_with_host`](crate::create_window_with_host) with this is equivalent
    /// to just calling [`create_window`](crate::create_window).
    #[inline]
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            main_thread: None,
            callbacks: None,
        }
    }

    /// Sets the [`HostMainThreadCaller`] handler to be used.
    ///
    /// If another callback handler was already set, it is replaced.
    ///
    /// # Platform Compatibility notes
    ///
    /// This is only useful on X11. On Window and macOS, this is a no-op.
    #[inline]
    #[allow(unused)]
    pub fn with_main_thread(mut self, main_thread: impl HostMainThreadCaller) -> Self {
        #[cfg(target_os = "linux")]
        {
            self.main_thread = Some(Box::new(main_thread));
        }

        self
    }

    /// Sets the [`HostCallbacks`] handler to be used.
    ///
    /// If another callback handler was already set, it is replaced.
    #[inline]
    pub fn with_callbacks(mut self, callbacks: impl HostCallbacks) -> Self {
        self.callbacks = Some(RefCell::new(Box::new(callbacks)));
        self
    }
}

#[allow(unused)]
impl Host {
    pub(crate) fn notify_destroyed(&self) {
        let Some(callbacks) = &self.callbacks else { return };
        let Ok(mut callbacks) = callbacks.try_borrow_mut() else { return };
        callbacks.destroyed();
    }

    pub(crate) fn request_resize(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        let Some(callbacks) = &self.callbacks else { return Ok(()) };
        let Ok(mut callbacks) = callbacks.try_borrow_mut() else { return Ok(()) };
        callbacks.request_resize(new_size)
    }
}
