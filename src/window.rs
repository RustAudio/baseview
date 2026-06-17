use crate::context::WindowContext;
use crate::handler::WindowHandler;
use crate::platform;
use crate::window_open_options::WindowOpenOptions;
use raw_window_handle::HasWindowHandle;
use std::marker::PhantomData;

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
    pub fn open_parented<H>(
        parent: &impl HasWindowHandle, options: WindowOpenOptions,
        build: impl for<'a> FnOnce(WindowContext<'a>) -> H + Send + 'static,
    ) -> WindowHandle
    where
        H: for<'a> WindowHandler<'a>,
    {
        let window_handle = platform::Window::open_parented(parent, options, build);
        WindowHandle::new(window_handle)
    }

    pub fn open_blocking<H>(
        options: WindowOpenOptions,
        build: impl for<'a> FnOnce(WindowContext<'a>) -> H + Send + 'static,
    ) where
        H: for<'a> WindowHandler<'a>,
    {
        platform::Window::open_blocking(options, build)
    }
}
