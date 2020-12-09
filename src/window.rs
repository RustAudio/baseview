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

pub struct WindowHandle(pub(crate) platform::WindowHandle);

pub struct Window<'a>(pub(crate) &'a mut platform::Window);

impl<'a> Window<'a> {
    pub fn open<H, B>(
        options: WindowOpenOptions,
        build: B
    ) -> (WindowHandle, Option<AppRunner>)
        where H: WindowHandler + 'static,
              B: FnOnce(&mut Window) -> H,
              B: Send + 'static
    {
        platform::Window::open::<H, B>(options, build)
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}

// Compile-time API assertions
#[doc(hidden)]
mod assertions {
    use crate::{WindowHandle, WindowHandler, Event, Window};

    struct TestWindowHandler {
        #[allow(dead_code)]
        ptr: *mut ::std::ffi::c_void,
    }

    impl WindowHandler for TestWindowHandler {
        fn on_event(&mut self, _: &mut Window, _: Event) {}
        fn on_frame(&mut self) {}
    }

    // Assert that WindowHandle is Send even if WindowHandler isn't
    static_assertions::assert_not_impl_any!(TestWindowHandler: Send);
    static_assertions::assert_impl_all!(WindowHandle: Send);
}
