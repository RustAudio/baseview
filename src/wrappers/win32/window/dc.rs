use crate::wrappers::win32::window::HWnd;
use std::ffi::c_void;
use std::ptr::NonNull;
use windows_core::{Error, Result};
use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC};

pub struct DeviceContext {
    window: HWnd,
    inner: NonNull<c_void>,
}

impl DeviceContext {
    pub(super) fn from_window(window: HWnd) -> Result<Self> {
        let dc = unsafe { GetDC(window.as_raw()) };

        let dc = NonNull::new(dc).ok_or_else(Error::from_thread)?;

        Ok(Self { window, inner: dc })
    }
}

impl Drop for DeviceContext {
    fn drop(&mut self) {
        unsafe { ReleaseDC(self.window.as_raw(), self.inner.as_ptr()) };
    }
}
