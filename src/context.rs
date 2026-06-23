use crate::{platform, MouseCursor, WindowSize};
use dpi::{Pixel, Size};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use std::fmt::Debug;

#[derive(Clone)]
pub struct WindowContext {
    inner: platform::WindowContext,
}

impl WindowContext {
    pub(crate) fn new(inner: platform::WindowContext) -> Self {
        Self { inner }
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        self.inner.set_mouse_cursor(mouse_cursor);
    }

    pub fn close(&self) {
        self.inner.close();
    }

    pub fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }

    pub fn focus(&self) {
        self.inner.focus();
    }

    pub fn resize<P: Pixel>(&self, size: impl Into<Size>) {
        self.inner.resize(size.into());
    }

    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    pub fn size(&self) -> WindowSize {
        self.inner.size()
    }

    pub fn platform_handle(&self) -> PlatformHandle {
        PlatformHandle { inner: self.inner.platform_handle() }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        self.inner.gl_context()
    }
}

impl HasWindowHandle for WindowContext {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle().ok_or(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for WindowContext {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(self.inner.display_handle())
    }
}

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
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle().ok_or(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for PlatformHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(self.inner.display_handle())
    }
}

impl Debug for PlatformHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
