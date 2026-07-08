mod xcb_connection;

use raw_window_handle::{DisplayHandle, HasWindowHandle, RawWindowHandle, XcbWindowHandle};
use std::fmt::Formatter;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::Arc;
pub(crate) use xcb_connection::X11Connection;

mod window;
pub use window::*;

mod cursor;
mod drag_n_drop;
mod event_loop;
mod keyboard;
mod visual_info;
mod window_thread;
mod window_thread_channel;

mod window_shared;

use crate::platform::x11::window_shared::WindowInner;
use crate::wrappers::xlib::XlibXcbConnection;

pub type WindowContext = Rc<WindowInner>;

#[cfg(feature = "opengl")]
pub mod gl;

#[derive(Clone)]
pub struct PlatformHandle {
    connection: Arc<XlibXcbConnection>,
    window_id: NonZero<x11rb::protocol::xproto::Window>,
    visual_id: NonZero<x11rb::protocol::xproto::Visualid>,
}

impl PlatformHandle {
    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let mut handle = XcbWindowHandle::new(self.window_id);
        handle.visual_id = Some(self.visual_id);
        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        self.connection.xcb_display_handle()
    }
}

impl std::fmt::Debug for PlatformHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display_string = self.connection.xlib_connection().display_string();
        let display_string: &str = &display_string.to_string_lossy();

        f.debug_struct("PlatformHandle (X11)")
            .field("connection", &display_string)
            .field("window_id", &self.window_id.get())
            .finish()
    }
}

pub struct ParentWindowHandle {
    window_id: u32,
}

impl ParentWindowHandle {
    pub fn extract(window: &impl HasWindowHandle) -> Self {
        let window_id = match window.window_handle().unwrap().as_raw() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window.get(),
            h => panic!("unsupported parent handle type {:?}", h),
        };

        Self { window_id }
    }
}
