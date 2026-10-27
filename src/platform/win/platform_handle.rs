use crate::wrappers::win32::h_instance::HInstance;
use raw_window_handle::{DisplayHandle, Win32WindowHandle};
use std::fmt::{Debug, Formatter};
use std::num::NonZeroIsize;

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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformHandle (Win32)")
            .field("hwnd", &self.hwnd.get())
            .field("hinstance", &HInstance::get_from_dll().addr().get())
            .finish()
    }
}
