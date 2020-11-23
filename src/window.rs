use crate::WindowHandler;
use crate::window_open_options::WindowOpenOptions;

#[cfg(target_os = "windows")]
use crate::win as platform;
#[cfg(target_os = "linux")]
use crate::x11 as platform;
#[cfg(target_os = "macos")]
use crate::macos as platform;


pub struct WindowHandle(platform::WindowHandle);


impl WindowHandle {
    pub fn app_run_blocking(self){
        self.0.app_run_blocking();
    }
}


pub struct Window<'a>(pub(crate) &'a mut platform::Window);


impl <'a>Window<'a> {
    pub fn open<H, B>(
        options: WindowOpenOptions,
        build: B
    ) -> WindowHandle
        where H: WindowHandler,
              B: FnOnce(&mut Window) -> H,
              B: Send + 'static
    {
        WindowHandle(platform::Window::open::<H, B>(options, build))
    }
}