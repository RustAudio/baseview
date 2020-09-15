use raw_window_handle::RawWindowHandle;

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
mod keyboard;
mod mouse_cursor;
pub use event::*;
pub use keyboard::*;
pub use mouse_cursor::MouseCursor;

pub enum Parent {
    None,
    AsIfParented,
    WithParent(RawWindowHandle),
}

unsafe impl Send for Parent {}

pub struct WindowOpenOptions {
    pub title: String,

    pub width: u32,
    pub height: u32,

    pub parent: Parent,

    pub frame_interval_secs: f64,
}

impl Default for WindowOpenOptions {
    fn default() -> Self {
        Self {
            title: String::from("baseview"),
            width: 640,
            height: 480,
            parent: Parent::None,
            frame_interval_secs: 15.0 / 1000.0,
        }
    }
}

pub trait WindowHandler {
    type Message;

    fn build(window: &mut Window) -> Self;

    fn on_frame(&mut self);
    fn on_event(&mut self, window: &mut Window, event: Event);
    fn on_message(&mut self, window: &mut Window, message: Self::Message);
}
