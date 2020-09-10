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
pub use event::*;
pub use key_code::KeyCode;
pub use mouse_cursor::MouseCursor;

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

    // The frequency to call the `draw()` method in calls per second.
    pub frame_rate: f64,
}

pub trait AppWindow {
    type AppMessage;

    fn build(window_handle: RawWindow, window_info: &WindowInfo) -> Self;

    fn draw(&mut self, mouse_cursor: &mut MouseCursor);
    fn on_event(&mut self, event: Event);
    fn on_app_message(&mut self, message: Self::AppMessage);

    /// The requested frequency to call `draw()` in calls per second.
    /// Set this to `None` to use the frame rate the host provides. (default is `None`)
    fn frame_rate() -> Option<f64> {
        None
    }

    /// The frequency to be sent `Event::Interval` in calls per second.
    /// Set this to `None` to not be sent this event. (default is `None`)
    fn interval() -> Option<f64> {
        None
    }
}

/// A wrapper for a `RawWindowHandle`. Some context creators expect an `&impl HasRawWindowHandle`.
#[derive(Debug, Copy, Clone)]
pub struct RawWindow {
    pub raw_window_handle: raw_window_handle::RawWindowHandle,
}

unsafe impl raw_window_handle::HasRawWindowHandle for RawWindow {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.raw_window_handle
    }
}
