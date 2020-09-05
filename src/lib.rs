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
pub use event::*;

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

    fn create_context(&mut self, window: RawWindow, window_info: &WindowInfo);
    fn draw(&mut self);
    fn on_event(&mut self, event: Event);
    fn on_app_message(&mut self, message: Self::AppMessage);
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
