mod drop_target;
mod hook;
mod keyboard;
mod window;
mod window_state;

use crate::wrappers::win32::h_instance::HInstance;
use raw_window_handle::{DisplayHandle, Win32WindowHandle};
use std::fmt::Debug;
use std::num::NonZeroIsize;
use std::rc::Rc;
pub use window::*;

#[cfg(feature = "opengl")]
pub mod gl;

pub type WindowContext = Rc<window_state::WindowState>;

#[derive(Clone)]
pub struct PlatformHandle {
    hwnd: NonZeroIsize,
}

impl PlatformHandle {
    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let mut handle = Win32WindowHandle::new(self.hwnd);
        handle.hinstance = Some(HInstance::get_from_dll().addr());

        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::windows()
    }
}

impl Debug for PlatformHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformHandle (Win32)")
            .field("hwnd", &self.hwnd.get())
            .field("hinstance", &HInstance::get_from_dll().addr().get())
            .finish()
    }
}
