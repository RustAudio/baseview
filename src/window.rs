use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::WindowHandler;
use crate::window_open_options::WindowOpenOptions;

#[cfg(target_os = "windows")]
use crate::win as platform;
#[cfg(target_os = "linux")]
use crate::x11 as platform;
#[cfg(target_os = "macos")]
use crate::macos as platform;

pub struct AppRunner(pub(crate) platform::AppRunner);

impl AppRunner {
    pub fn app_run_blocking(self){
        self.0.app_run_blocking();
    }
}

pub struct Window<'a>(pub(crate) &'a mut platform::Window);

impl<'a> Window<'a> {
    pub fn open<H, B>(
        options: WindowOpenOptions,
        build: B
    ) -> Option<AppRunner>
        where H: WindowHandler + 'static,
              B: FnOnce(&mut Window) -> H,
              B: Send + 'static
    {
        platform::Window::open::<H, B>(options, build)
    }

    // Captures the mouse cursor for this window
    pub fn set_mouse_capture(&self) {
        platform::Window::set_mouse_capture(self.0);
    }

    // Returns true if this window has captured the mouse
    pub fn get_mouse_capture(&self) -> bool {
        platform::Window::get_mouse_capture(self.0)
    }

    // Releases the mouse capture from all windows
    pub fn release_mouse_capture(&self) {
        platform::Window::release_mouse_capture(self.0)
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}
