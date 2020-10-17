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
mod window_info;
mod window_open_options;
pub use event::*;
pub use keyboard::*;
pub use mouse_cursor::MouseCursor;
pub use window_info::WindowInfo;
pub use window_open_options::*;

#[derive(Debug)]
pub enum Parent {
    None,
    AsIfParented,
    WithParent(RawWindowHandle),
}

unsafe impl Send for Parent {}

pub struct WindowHandle {
    thread: std::thread::JoinHandle<()>,
}

impl WindowHandle {
    pub fn app_run_blocking(self) {
        let _ = self.thread.join();
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    pub fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl From<Point<f64>> for Point<i32> {
    fn from(p: Point<f64>) -> Point<i32> {
        Point::new(p.x.round() as i32, p.y.round() as i32)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

type WindowOpenResult = Result<WindowInfo, ()>;

pub trait WindowHandler {
    type Message;

    fn build(window: &mut Window) -> Self;

    fn on_frame(&mut self);
    fn on_event(&mut self, window: &mut Window, event: Event);
    fn on_message(&mut self, window: &mut Window, message: Self::Message);
}
