use crate::platform::macos::cursor::Cursor;
use crate::platform::macos::view::BaseviewView;
use crate::wrappers::appkit::{View, ViewRef};
use crate::{MouseCursor, Size};
use objc2::rc::Weak;
use objc2::runtime::NSObjectProtocol;
use objc2::Message;
use raw_window_handle::DisplayHandle;

#[derive(Clone)]
pub struct WindowContext {
    view: Weak<View<BaseviewView>>,
}

impl WindowContext {
    pub(crate) fn new(view: ViewRef<'_, BaseviewView>) -> Self {
        view.view.retain();
        Self { view: Weak::from_retained(&view.view.retain()) }
    }

    pub fn close(&self) {
        let Some(view) = self.view.load() else { return };
        BaseviewView::close(view.inner_ref());
    }

    pub fn has_focus(&self) -> bool {
        let Some(view) = self.view.load() else { return false };
        let Some(window) = view.window() else {
            return false;
        };

        if !window.isKeyWindow() {
            return false;
        }

        let Some(first_responder) = window.firstResponder() else {
            return false;
        };

        view.isEqual(Some(&*first_responder))
    }

    pub fn focus(&self) {
        let Some(view) = self.view.load() else { return };
        if let Some(window) = view.window() {
            window.makeFirstResponder(Some(&view));
        }
    }

    pub fn resize(&self, size: Size) {
        let Some(view) = self.view.load() else { return };
        let view = view.inner_ref();
        if view.inner.state.closed.get() {
            return;
        }

        BaseviewView::resize(view, size);
    }

    pub fn set_mouse_cursor(&self, cursor: MouseCursor) {
        let Some(view) = self.view.load() else { return };
        let native_cursor = Cursor::from(cursor);
        view.addCursorRect_cursor(view.bounds(), &native_cursor.load());
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        Some(crate::gl::GlContext::new(self.view.load()?.inner().gl_context.get()?.clone()))
    }

    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        View::window_handle_from_weak(&self.view)
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::appkit()
    }
}
