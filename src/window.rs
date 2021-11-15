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

pub struct WindowHandle {
    window_handle: platform::WindowHandle,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl WindowHandle {
    fn new(window_handle: platform::WindowHandle) -> Self {
        Self { window_handle, phantom: PhantomData::default() }
    }

    /// Close the window
    pub fn close(&mut self) {
        self.window_handle.close();
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    pub fn is_open(&self) -> bool {
        self.window_handle.is_open()
    }
}

pub trait WindowHandler {
    fn on_frame(&mut self, window: &mut Window);
    fn on_event(&mut self, window: &mut Window, event: Event) -> EventStatus;
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window_handle.raw_window_handle()
    }
}

pub struct Window<'a> {
    window: &'a mut platform::Window,
    // so that Window is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl<'a> Window<'a> {
    pub(crate) fn new(window: &mut platform::Window) -> Window {
        Window { window, phantom: PhantomData }
    }

    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        let window_handle = platform::Window::open_parented::<P, H, B>(parent, options, build);
        WindowHandle::new(window_handle)
    }

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        let window_handle = platform::Window::open_as_if_parented::<H, B>(options, build);
        WindowHandle::new(window_handle)
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        platform::Window::open_blocking::<H, B>(options, build)
    }

    /// Close the window
    pub fn close(&mut self) {
        self.window.close();
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }
}
