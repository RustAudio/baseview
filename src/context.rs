use crate::{platform, MouseCursor, Size};
use raw_window_handle::{HandleError, HasWindowHandle, WindowHandle};
use std::marker::PhantomData;

pub struct WindowContext<'a> {
    inner: platform::WindowContext,
    _marker: PhantomData<&'a ()>,
}

impl WindowContext<'_> {
    pub(crate) fn new(inner: platform::WindowContext) -> Self {
        Self { inner, _marker: PhantomData }
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

impl HasWindowHandle for WindowContext<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        todo!()
    }
}
