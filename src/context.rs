use super::*;
use crate::{platform, MouseCursor, WindowSize};
use dpi::Size;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use std::fmt::Debug;

/// A handle to the window given to a [`WindowHandler`](crate::WindowHandler), which it can then
/// use to perform various operations on the window itself.
#[derive(Clone)]
pub struct WindowContext {
    inner: platform::WindowContext,
}

impl WindowContext {
    pub(crate) fn new(inner: platform::WindowContext) -> Self {
        Self { inner }
    }

    /// Sets the [`MouseCursor`] icon to be displayed when the mouse cursor hovers this window.
    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) -> Result<(), Error> {
        self.inner.set_mouse_cursor(mouse_cursor)?;
        Ok(())
    }

    /// Requests the window to be closed.
    ///
    /// This request is not immediate. When exactly it will be closed is platform-dependent.
    ///
    /// However, it is guaranteed to only get closed after all [`WindowHandler`](crate::WindowHandler)
    /// methods are completed, and soon enough to be perceived as instantaneous by the user.
    pub fn request_close(&self) {
        self.inner.request_close();
    }

    /// Returns `true` if this window currently has keyboard focus, `false` otherwise.
    pub fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }

    /// Focuses this window.
    pub fn focus(&self) -> Result<(), Error> {
        self.inner.focus()?;
        Ok(())
    }

    /// Resizes this window to the given `size`.
    ///
    /// The given `size` can either be in [physical](dpi::PhysicalSize) or
    /// [logical](dpi::LogicalSize) pixels.
    pub fn resize(&self, size: impl Into<Size>) -> Result<(), Error> {
        self.inner.resize(size.into())?;
        Ok(())
    }

    /// Returns the current scale factor of this window.
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Returns the current size of this window.
    pub fn size(&self) -> WindowSize {
        self.inner.size()
    }

    /// Returns a new lightweight [`PlatformHandle`] to this window.
    ///
    /// It can be sent across threads to access the underlying platform window and display connection.
    /// See the [`PlatformHandle`] documentation for more information.
    pub fn platform_handle(&self) -> PlatformHandle {
        PlatformHandle { inner: self.inner.platform_handle() }
    }

    /// Returns the [`GlContext`](crate::gl::GlContext) associated to this window.
    ///
    /// If the window was not created with a GL context, this will return [`None`].
    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        self.inner.gl_context()
    }
}

impl HasWindowHandle for WindowContext {
    fn window_handle(&self) -> core::result::Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle().ok_or(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for WindowContext {
    fn display_handle(&self) -> core::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(self.inner.display_handle())
    }
}

/// A lightweight handle to a window and its display connection.
///
/// This type implements both [`HasWindowHandle`] and [`HasDisplayHandle`], allowing users to access
/// the underlying platform window and display connection.
///
/// This type is cheaply [cloneable](Clone). All cloned handles will always refer to the same window and
/// display connection.
///
/// Unlike the [`WindowContext`] and [`WindowHandle`] types however, this handle can be safely sent,
/// used, and dropped across threads (it implements [`Send`] and [`Sync`]).
///
/// However, it is not guaranteed that this handle can be successfully used from other threads,
/// depending on the platform.
///
/// Similarly, this handle is actually a weak handle: while it remains safe to use, its methods
/// will always return [`HandleError::Unavailable`] if called after the window it refers to has been
/// destroyed.
///
/// # Platform compatibility notes
///
/// Depending on the platform, the [`PlatformHandle::window_handle`] method may return
/// [`HandleError::Unavailable`] if called from a thread other than the main thread. (Even if the
/// window is still alive and well)
#[derive(Clone)]
pub struct PlatformHandle {
    inner: platform::PlatformHandle,
}

// Assert that PlatformHandle implements both Send & Sync on all platforms
const _: () = {
    const fn assert_impl_all<T: Debug + Send + Sync>() {}
    let _: fn() = assert_impl_all::<PlatformHandle>;
};

impl HasWindowHandle for PlatformHandle {
    fn window_handle(&self) -> core::result::Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle().ok_or(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for PlatformHandle {
    fn display_handle(&self) -> core::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(self.inner.display_handle())
    }
}

impl Debug for PlatformHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
