use crate::platform::macos::cursor::Cursor;
use crate::platform::macos::view::BaseviewView;
use crate::platform::Result;
use crate::platform::{PlatformHandle, WindowSharedState};
use crate::wrappers::appkit::{View, ViewRef};
use crate::*;
use dispatch2::MainThreadBound;
use dpi::Size;
use objc2::rc::Weak;
use objc2::runtime::NSObjectProtocol;
use objc2::{MainThreadMarker, Message};
use raw_window_handle::DisplayHandle;
use std::rc::Rc;

#[derive(Clone)]
pub struct WindowContext {
    mtm: MainThreadMarker,
    view: Weak<View<BaseviewView>>,
    state: Rc<WindowSharedState>,
}

impl WindowContext {
    pub(crate) fn new(view: ViewRef<'_, BaseviewView>) -> Self {
        Self {
            view: Weak::from_retained(&view.view.retain()),
            state: Rc::clone(&view.state),
            mtm: view.mtm,
        }
    }

    pub fn request_close(&self) {
        let Some(view) = self.view.load() else { return };
        let Some(view) = view.inner_ref() else { return };
        BaseviewView::close(view, false);
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

    pub fn focus(&self) -> Result<()> {
        let Some(view) = self.view.load() else { return Ok(()) };
        if let Some(window) = view.window() {
            window.makeFirstResponder(Some(&view));
        }

        Ok(())
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        let Some(view) = self.view.load() else { return Ok(()) };
        let Some(view) = view.inner_ref() else { return Ok(()) };
        if view.inner.state.closed.get() {
            return Ok(());
        }

        BaseviewView::resize(view, size, true);

        Ok(())
    }

    pub fn set_mouse_cursor(&self, cursor: MouseCursor) -> Result<()> {
        let Some(view) = self.view.load() else { return Ok(()) };
        let native_cursor = Cursor::from(cursor);
        view.addCursorRect_cursor(view.bounds(), &native_cursor.load());
        Ok(())
    }

    pub fn size(&self) -> WindowSize {
        WindowSize::from_logical(self.state.size.get(), self.state.scale_factor.get())
    }

    pub fn scale_factor(&self) -> f64 {
        self.state.scale_factor.get()
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        Some(crate::gl::GlContext::new(self.view.load()?.inner()?.gl_context.get()?.clone()))
    }

    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        View::window_handle_from_weak(&self.view)
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::appkit()
    }

    pub fn platform_handle(&self) -> PlatformHandle {
        PlatformHandle { inner: MainThreadBound::new(self.view.clone(), self.mtm) }
    }
}
