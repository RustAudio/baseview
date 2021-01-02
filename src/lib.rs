#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "macos")]
mod macos;

mod event;
mod keyboard;
mod mouse_cursor;
mod window;
mod window_info;
mod window_open_options;

pub use event::*;
pub use mouse_cursor::MouseCursor;
pub use window::*;
pub use window_info::*;
pub use window_open_options::*;
