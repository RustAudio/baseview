use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::WindowHandler;
use crate::window_open_options::WindowOpenOptions;

#[cfg(target_os = "windows")]
use crate::win as platform;
#[cfg(target_os = "linux")]
use crate::x11 as platform;
#[cfg(target_os = "macos")]
use crate::macos as platform;


pub struct WindowHandle<H: WindowHandler>(pub(crate) platform::WindowHandle<H>);


// Implement Clone manually to avoid H: Clone bound
impl <H: WindowHandler>Clone for WindowHandle<H> {
    fn clone(&self) -> Self {
        WindowHandle(self.0.clone())
    }
}


impl <H: WindowHandler>WindowHandle<H> {
    pub fn app_run_blocking(self){
        self.0.app_run_blocking();
    }

    pub fn try_send_message(
        &self,
        message: H::Message
    ) -> Result<(), H::Message> {
        self.0.try_send_message(message)
    }
}


pub struct Window<'a>(pub(crate) &'a mut platform::Window);


impl <'a>Window<'a> {
    pub fn open<H, B>(
        options: WindowOpenOptions,
        build: B
    ) -> WindowHandle<H>
        where H: WindowHandler,
              B: FnOnce(&mut Window) -> H,
              B: Send + 'static
    {
        platform::Window::open::<H, B>(options, build)
    }
}


unsafe impl <'a>HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}