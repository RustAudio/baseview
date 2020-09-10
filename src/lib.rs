use std::ffi::c_void;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::*;

#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
pub use crate::x11::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

mod event;
mod key_code;
mod mouse_cursor;
mod window_state;
pub use event::*;
pub use key_code::KeyCode;
pub use mouse_cursor::MouseCursor;
pub use window_state::WindowState;

pub enum Parent {
    None,
    AsIfParented,
    WithParent(*mut c_void),
}

pub struct WindowOpenOptions<'a> {
    pub title: &'a str,

    pub width: usize,
    pub height: usize,

    pub parent: Parent,
}

pub trait AppWindow {
    type AppMessage;

    fn build(window: &mut WindowState) -> Self;

    fn draw(&mut self);
    fn on_event(&mut self, event: Event, window: &mut WindowState);
    fn on_app_message(&mut self, message: Self::AppMessage, window: &mut WindowState);
}
