use crate::dpi::WindowSize;
use std::error::Error;
use std::time::Duration;

/// A special handler for the Window thread to wake up and call methods on the main thread.
///
/// [`HostedWindow::host_main_thread_callback`](crate::HostedWindow::host_main_thread_callback)
/// should be called as a response to this.
///
/// # Platform compatibility notes
///
/// This is only needed on X11, as Windows and macOS windows already run on the main thread.
pub trait HostMainThreadCaller: Send + 'static {
    /// Schedules a callback on the main thread.
    ///
    /// [`HostedWindow::host_main_thread_callback`](crate::HostedWindow::host_main_thread_callback)
    /// should be called as a response to this.
    ///
    /// # Platform compatibility notes
    ///
    /// Only X11 needs this. This can be implemented as a no-op on Windows and macOS.
    fn call_main_thread(&mut self);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimerHandle(pub u32);

pub trait HostTimerSupport: 'static {
    fn register_timer(&mut self, period: Duration) -> Result<TimerHandle, Box<dyn Error>>;
    fn unregister_timer(&mut self, timer: TimerHandle) -> Result<(), Box<dyn Error>>;
}

#[cfg(unix)]
use std::os::fd::*;
pub trait HostFdSupport: 'static {
    #[cfg(unix)]
    fn register_fd(&self, fd: BorrowedFd) -> Result<(), Box<dyn Error>>;
    #[cfg(unix)]
    fn unregister_fd(&self, fd: RawFd) -> Result<(), Box<dyn Error>>;
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
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), Box<dyn Error>>;
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
/// It also brings the additional safety guarantee that all handlers given to this type will be
/// destroyed alongside with the window.
///
/// This guarantees callbacks cannot be fired after the window is dropped.
/// (or after this [`Host`] object is dropped, if it never made it to a window creation call).
pub struct Host {
    callbacks: Option<Box<dyn HostCallbacks>>,
    #[cfg(target_os = "linux")]
    main_thread: Option<Box<dyn HostMainThreadCaller>>,
    #[cfg(target_os = "linux")]
    timer: Option<Box<dyn HostTimerSupport>>,
    #[cfg(target_os = "linux")]
    fd: Option<Box<dyn HostFdSupport>>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    /// Creates a new, empty host with no callbacks.
    #[inline]
    pub fn new() -> Self {
        Self {
            callbacks: None,
            #[cfg(target_os = "linux")]
            main_thread: None,
            #[cfg(target_os = "linux")]
            timer: None,
            #[cfg(target_os = "linux")]
            fd: None,
        }
    }

    /// Sets the [`HostMainThreadCaller`] handler to be used.
    ///
    /// If another callback handler was already set, it is replaced.
    ///
    /// # Platform Compatibility notes
    ///
    /// This is only useful on X11. On Windows and macOS, this is a no-op.
    #[inline]
    pub fn with_main_thread(self, main_thread: impl HostMainThreadCaller) -> Self {
        #[cfg(target_os = "linux")]
        {
            let mut this = self;
            this.main_thread = Some(Box::new(main_thread));
            this
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = main_thread;
            self
        }
    }

    #[inline]
    pub fn with_fd(self, fd: impl HostFdSupport) -> Self {
        #[cfg(target_os = "linux")]
        {
            let mut this = self;
            this.fd = Some(Box::new(fd));
            this
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = fd;
            self
        }
    }

    #[inline]
    pub fn with_timer(self, timer: impl HostTimerSupport) -> Self {
        #[cfg(target_os = "linux")]
        {
            let mut this = self;
            this.timer = Some(Box::new(timer));
            this
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = timer;
            self
        }
    }

    /// Sets the [`HostCallbacks`] handler to be used.
    ///
    /// If another callback handler was already set, it is replaced.
    #[inline]
    pub fn with_callbacks(mut self, callbacks: impl HostCallbacks) -> Self {
        self.callbacks = Some(Box::new(callbacks));
        self
    }

    #[inline]
    pub fn take_callbacks(&mut self) -> Option<Box<dyn HostCallbacks>> {
        self.callbacks.take()
    }

    #[inline]
    pub fn take_main_thread(&mut self) -> Option<Box<dyn HostMainThreadCaller>> {
        #[cfg(target_os = "linux")]
        {
            self.main_thread.take()
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
}
