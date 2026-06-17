use crate::{platform, MouseCursor, Size};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

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

    pub fn resize(&mut self, size: Size) {
        self.inner.resize(size);
    }
}

impl HasWindowHandle for WindowContext {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(self.inner.window_handle())
    }
}

impl HasDisplayHandle for WindowContext {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(self.inner.display_handle())
    }
}
