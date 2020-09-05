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

    fn create_context(
        &mut self,
        window: raw_window_handle::RawWindowHandle,
        window_info: &WindowInfo,
    );
    fn draw(&mut self);
    fn on_event(&mut self, event: Event);
    fn on_app_message(&mut self, message: Self::AppMessage);
}
