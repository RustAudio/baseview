use std::marker::PhantomData;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::event::{Event, EventStatus};
use crate::window_open_options::WindowOpenOptions;

#[cfg(target_os = "macos")]
use crate::macos as platform;
#[cfg(target_os = "windows")]
use crate::win as platform;
#[cfg(target_os = "linux")]
use crate::x11 as platform;

pub use platform::HostWindowHandle;

pub trait WindowHandler {
    fn on_frame(&mut self, window: &mut Window);
    fn on_event(&mut self, window: &mut Window, event: Event) -> EventStatus;
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

    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> HostWindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        platform::Window::open_parented::<P, H, B>(parent, options, build)
    }

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> (RawWindowHandle, HostWindowHandle)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        platform::Window::open_as_if_parented::<H, B>(options, build)
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        platform::Window::open_blocking::<H, B>(options, build)
    }

    pub fn request_close(&mut self) {
        self.window.request_close();
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }
}
