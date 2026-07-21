mod xcb_connection;

use raw_window_handle::{
    DisplayHandle, HandleError, HasWindowHandle, RawWindowHandle, XcbWindowHandle,
};
use std::fmt::{Display, Formatter};
use std::num::{NonZero, NonZeroU32, TryFromIntError};
use std::rc::Rc;
use std::sync::Arc;
pub(crate) use xcb_connection::X11Connection;

mod window;
pub use window::*;

mod cursor;
mod drag_n_drop;
mod error;
mod event_loop;
mod keyboard;
mod visual_info;
mod xcb_window;

mod window_shared;
mod window_thread;

pub use error::{CookieExt as _, Error};
pub(crate) type Result<T> = std::result::Result<T, Error>;

use crate::platform::x11::window_shared::WindowInner;
use crate::wrappers::xlib::XlibXcbConnection;

pub type WindowContext = Rc<WindowInner>;

#[cfg(feature = "opengl")]
pub mod gl;

#[derive(Clone)]
pub struct PlatformHandle {
    connection: Arc<XlibXcbConnection>,
    window_id: NonZero<x11rb::protocol::xproto::Window>,
    visual_id: x11rb::protocol::xproto::Visualid,
}

impl PlatformHandle {
    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let mut handle = XcbWindowHandle::new(self.window_id);
        handle.visual_id = NonZero::new(self.visual_id);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentWindowHandle {
    window_id: NonZeroU32,
}

impl ParentWindowHandle {
    pub fn extract(
        window: &impl HasWindowHandle,
    ) -> core::result::Result<Self, ParentWindowHandleError> {
        let window_id = match window.window_handle()?.as_raw() {
            RawWindowHandle::Xlib(h) => {
                NonZeroU32::new(h.window.try_into()?).ok_or(ParentWindowHandleError::NullId)?
            }
            RawWindowHandle::Xcb(h) => h.window,
            h => Err(ParentWindowHandleError::UnsupportedWindowHandleType(h))?,
        };

        Ok(Self { window_id })
    }
}

pub enum ParentWindowHandleError {
    HandleError(HandleError),
    UnsupportedWindowHandleType(RawWindowHandle),
    InvalidU32(TryFromIntError),
    NullId,
}

impl From<HandleError> for ParentWindowHandleError {
    fn from(value: HandleError) -> Self {
        Self::HandleError(value)
    }
}

impl From<TryFromIntError> for ParentWindowHandleError {
    fn from(value: TryFromIntError) -> Self {
        Self::InvalidU32(value)
    }
}

impl Display for ParentWindowHandleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParentWindowHandleError::HandleError(e) => e.fmt(f),
            ParentWindowHandleError::UnsupportedWindowHandleType(h) => {
                write!(f, "Unsupported window handle type on X11: {h:?}")
            }
            ParentWindowHandleError::InvalidU32(e) => write!(f, "Invalid window XID: {e}"),
            ParentWindowHandleError::NullId => f.write_str("Window XID is zero"),
        }
    }
}
