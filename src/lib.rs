// This is required because the objc crate is causing a lot of warnings: https://github.com/SSheldon/rust-objc/issues/125
// Eventually we should migrate to the objc2 crate and remove this.
#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "linux")]
pub(crate) mod x11;

mod clipboard;
mod event;
mod keyboard;
mod mouse_cursor;
mod window;
mod window_info;
mod window_open_options;

#[cfg(feature = "opengl")]
pub mod gl;

pub use clipboard::*;
pub use event::*;
pub use mouse_cursor::MouseCursor;
pub use window::*;
pub use window_info::*;
pub use window_open_options::*;
