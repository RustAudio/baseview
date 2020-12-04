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


pub struct WindowHandle<H: WindowHandler>(
    pub(crate) platform::WindowHandle<H>
);


impl <H: WindowHandler>WindowHandle<H> {
    pub fn try_send_message(
        &mut self,
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
    ) -> (WindowHandle<H>, Option<AppRunner>)
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


// Compile-time API assertions
#[doc(hidden)]
mod assertions {
    use crate::{WindowHandle, WindowHandler, Event, Window};

    struct TestWindowHandler {
        #[allow(dead_code)]
        ptr: *mut ::std::ffi::c_void,
    }

    impl WindowHandler for TestWindowHandler {
        type Message = ();

        fn on_event(&mut self, _: &mut Window, _: Event) {}
        fn on_message(&mut self, _: &mut Window, _: Self::Message) {}
        fn on_frame(&mut self) {}
    }

    // Assert that WindowHandle is Send even if WindowHandler isn't
    static_assertions::assert_not_impl_any!(TestWindowHandler: Send);
    static_assertions::assert_impl_all!(WindowHandle<TestWindowHandler>: Send);
}
