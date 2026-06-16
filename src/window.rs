use raw_window_handle::{HasRawWindowHandle, HasWindowHandle};
use std::marker::PhantomData;
use std::process::Output;

use crate::event::{Event, EventStatus};
use crate::window_open_options::WindowOpenOptions;
use crate::{platform, MouseCursor, Size};

pub struct WindowHandle {
    window_handle: platform::WindowHandle,
    // so that WindowHandle is !Send on all platforms
    phantom: PhantomData<*mut ()>,
}

impl WindowHandle {
    fn new(window_handle: platform::WindowHandle) -> Self {
        Self { window_handle, phantom: PhantomData }
    }

    /// Close the window
    pub fn close(&self) {
        self.window_handle.close();
    }

    /// Returns `true` if the window is still open, and returns `false`
    /// if the window was closed/dropped.
    pub fn is_open(&self) -> bool {
        self.window_handle.is_open()
    }
}

pub struct Window {
    _private: (),
}

impl Window {
    pub fn open_parented<P, H, B>(
        parent: &impl HasWindowHandle, options: WindowOpenOptions,
        build: impl for<'a> FnOnce(WindowContext<'a>) -> H<'a>,
    ) -> WindowHandle
    where
        H: for<'a> WindowHandler<'a>,
        B: FnOnce(&mut Window) -> H,
        B: Send + 'static,
    {
        let window_handle = platform::Window::open_parented::<P, H, B>(parent, options, build);
        WindowHandle::new(window_handle)
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: for<'a> WindowHandler<'a>,
        B: FnOnce(WindowContext) -> H,
        B: Send + 'static,
    {
        platform::Window::open_blocking::<H, B>(options, build)
    }
}
