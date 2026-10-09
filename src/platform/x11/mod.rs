mod cursor;
mod drag_n_drop;
mod error;
mod event_loop;
#[cfg(feature = "opengl")]
pub mod gl;
mod handler;
mod host_handle;
mod keyboard;
mod mouse;
mod present;
mod sizing;
mod timer;
mod visual_info;
mod waker;
mod window_handle;
mod window_shared;
// mod window_thread;
mod x11_connection;
mod xcb_window;

pub(crate) mod prelude {
    pub use super::cursor::*;
    pub use super::error::{CookieExt as _, FatalError, PlatformError, ReplyExt as _};
    pub use super::event_loop::*;
    pub use super::handler::Handler;
    pub use super::mouse::*;
    pub use super::present::*;
    pub use super::sizing::*;
    pub use super::timer::*;
    pub use super::visual_info::WindowVisualConfig;
    pub use super::waker::WindowWaker;
    pub use super::window_shared::WindowShared;
    pub use super::x11_connection::X11Connection;
    pub use super::xcb_window::XcbWindow;
    pub(crate) use crate::{dpi::*, tracing::*, MouseCursor, WindowSettings, WindowSize};
    pub use std::cell::Cell;
    pub use std::rc::Rc;
    pub use std::sync::Arc;
    pub use x11rb::connection::Connection;
    pub use x11rb::errors::{ConnectionError, ReplyOrIdError};
    pub use x11rb::protocol::xproto::{ConnectionExt as _, Cursor};
    pub use x11rb::xcb_ffi::XCBConnection;
    pub type PlatformResult<T> = Result<T, PlatformError>;
    #[cfg(feature = "opengl")]
    pub use super::gl::{GlContextInner, GlCreationFailedError, PlatformGlContext};
    #[cfg(feature = "opengl")]
    pub use crate::gl::{GlConfig, GlContext};
}

use prelude::*;

use crate::wrappers::xlib::XlibXcbConnection;
use raw_window_handle::{
    DisplayHandle, HandleError, HasWindowHandle, RawWindowHandle, XcbWindowHandle,
};
use std::fmt::{Display, Formatter};
use std::num::{NonZero, NonZeroU32, TryFromIntError};

pub type WindowContext = Rc<WindowShared>;
pub use error::PlatformError;
pub use present::DamageRect;
pub use timer::TimerHandle;
pub use waker::WindowWaker;
pub type WindowHandle = EventLoop;

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
        window: &(impl HasWindowHandle + ?Sized),
    ) -> core::result::Result<Self, ParentWindowHandleError> {
        let window_id = match window.window_handle()?.as_raw() {
            RawWindowHandle::Xlib(h) => {
                NonZeroU32::new(h.window.try_into()?).ok_or(ParentWindowHandleError::NullId)?
            }
            RawWindowHandle::Xcb(h) => h.window,
            h => return Err(ParentWindowHandleError::UnsupportedWindowHandleType(h)),
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

#[inline]
pub fn assume_standalone_in_process() {
    // No-op on X11
}

pub fn copy_to_clipboard(_data: &str) {
    unimplemented!()
}
