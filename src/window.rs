use std::marker::PhantomData;

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

pub struct Window<'a> {
    window: &'a mut platform::Window,
    // so that Window is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl<'a> Window<'a> {
    pub(crate) fn new(window: &mut platform::Window) -> Window {
        Window {
            window,
            phantom: PhantomData,
        }
    }

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
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }
}
