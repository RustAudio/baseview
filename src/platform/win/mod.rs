mod drop_target;
mod error;
mod hook;
mod keyboard;
mod window;
mod window_state;

use crate::wrappers::win32::h_instance::HInstance;
use crate::wrappers::win32::window::HWnd;
pub use error::{Error, Result};
use raw_window_handle::{
    DisplayHandle, HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle,
};
use std::fmt::{Debug, Display, Formatter};
use std::num::NonZeroIsize;
use std::ptr::NonNull;
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

#[derive(Debug, Clone, PartialEq)]
pub struct ParentWindowHandle {
    handle: HWnd,
}

impl ParentWindowHandle {
    pub fn extract(
        parent: &impl HasWindowHandle,
    ) -> core::result::Result<Self, ParentWindowHandleError> {
        let parent = match parent.window_handle()?.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd,
            h => return Err(ParentWindowHandleError::UnsupportedWindowHandleType(h)),
        };

        let parent = NonNull::new(parent.get() as _).unwrap();

        Ok(Self { handle: unsafe { HWnd::from_raw(parent) } })
    }
}

pub enum ParentWindowHandleError {
    HandleError(HandleError),
    UnsupportedWindowHandleType(RawWindowHandle),
}

impl From<HandleError> for ParentWindowHandleError {
    fn from(err: HandleError) -> Self {
        Self::HandleError(err)
    }
}

impl Display for ParentWindowHandleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParentWindowHandleError::HandleError(e) => Display::fmt(e, f),
            ParentWindowHandleError::UnsupportedWindowHandleType(h) => {
                write!(f, "Unsupported window handle type on Win32: {h:?}")
            }
        }
    }
}
