use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::event::{Event, EventStatus};
use crate::window_open_options::WindowOpenOptions;
use crate::{MouseCursor, Size};

#[cfg(target_os = "macos")]
use crate::macos as platform;
#[cfg(target_os = "windows")]
use crate::win as platform;
#[cfg(target_os = "linux")]
use crate::x11 as platform;

pub struct WindowHandle {
    window_handle: platform::WindowHandle,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl WindowHandle {
    fn new(window_handle: platform::WindowHandle) -> Self {
        Self { window_handle, phantom: PhantomData }
    }

    /// Close the window
    pub fn close(&mut self) {
        self.window_handle.close();
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    pub fn is_open(&self) -> bool {
        self.window_handle.is_open()
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window_handle.raw_window_handle()
    }
}

pub trait WindowHandler: 'static {
    fn on_frame(&mut self);
    fn on_event(&mut self, event: Event) -> EventStatus;
}

#[derive(Clone)]
pub struct Window {
    window: Weak<platform::Window>,
}

impl Window {
    #[inline]
    pub(crate) fn new(window: Weak<platform::Window>) -> Window {
        Window { window }
    }

    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(Window) -> H,
        B: Send + 'static,
    {
        let window_handle = platform::Window::open_parented(parent, options, build);
        WindowHandle::new(window_handle)
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(Window) -> H,
        B: Send + 'static,
    {
        platform::Window::open_blocking(options, build)
    }

    fn inner(&self) -> Rc<platform::Window> {
        self.window.upgrade().expect("Window has already been destroyed")
    }

    /// Close the window
    pub fn close(&self) {
        // This can be a no-op if the window is already closed.
        if let Some(window) = self.window.upgrade() {
            window.close()
        }
    }

    /// Resize the window to the given size. The size is always in logical pixels. DPI scaling will
    /// automatically be accounted for.
    pub fn resize(&self, size: Size) {
        self.inner().resize(size);
    }

    pub fn set_mouse_cursor(&self, cursor: MouseCursor) {
        self.inner().set_mouse_cursor(cursor);
    }

    pub fn has_focus(&self) -> bool {
        self.inner().has_focus()
    }

    pub fn focus(&self) {
        self.inner().focus()
    }

    /// If provided, then an OpenGL context will be created for this window. You'll be able to
    /// access this context through [crate::Window::gl_context].
    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        Some(crate::gl::GlContext::new(self.inner().gl_context()?))
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.inner().raw_window_handle()
    }
}

unsafe impl HasRawDisplayHandle for Window {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        self.inner().raw_display_handle()
    }
}
