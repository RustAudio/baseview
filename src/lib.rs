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
pub use event::*;
pub use keyboard::*;
pub use mouse_cursor::MouseCursor;
pub use window_info::WindowInfo;

pub enum Parent {
    None,
    AsIfParented,
    WithParent(RawWindowHandle),
}

unsafe impl Send for Parent {}

pub enum WindowResize {
    None,
    MinMax {
        min_logical_size: (u32, u32),
        max_logical_size: (u32, u32),
        keep_aspect: bool,
    },
}

pub struct WindowOpenOptions {
    pub title: String,

    /// The logical width and height of the window
    pub logical_size: (u32, u32),

    pub resize: WindowResize,

    /// The dpi scale factor. This will used in conjunction with the dpi scale
    /// factor of the system.
    pub scale: f64,

    pub parent: Parent,
}

pub struct WindowHandle {
    thread: std::thread::JoinHandle<()>,
}

impl WindowHandle {
    pub fn app_run_blocking(self) {
        let _ = self.thread.join();
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
