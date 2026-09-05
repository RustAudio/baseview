mod clipboard;
mod context;
pub mod dpi;
mod error;
mod event;
mod handler;
pub mod host;
mod keyboard;
mod mouse_cursor;
mod settings;
mod tracing;
mod window;

pub(crate) mod platform;

#[cfg(feature = "opengl")]
pub mod gl;

pub use clipboard::*;
pub use context::{PlatformHandle, WindowContext};
pub use error::*;
pub use event::*;
pub use handler::WindowHandler;
pub use mouse_cursor::MouseCursor;
pub use settings::*;
pub use window::*;

#[allow(unused, reason = "Some platforms may not use all exports from this mod")]
pub(crate) use tracing::*;

mod utils;
pub(crate) mod wrappers;

#[inline]
pub unsafe fn assume_standalone_in_process() {
    platform::assume_standalone_in_process()
}
