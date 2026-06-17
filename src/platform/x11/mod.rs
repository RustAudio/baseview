mod xcb_connection;

use std::rc::Rc;
pub(crate) use xcb_connection::XcbConnection;

mod window;
pub use window::*;

mod cursor;
mod drag_n_drop;
mod event_loop;
mod keyboard;
mod visual_info;

mod window_shared;

use crate::platform::x11::window_shared::WindowInner;
pub type WindowContext = Rc<WindowInner>;

#[cfg(feature = "opengl")]
pub mod gl;
