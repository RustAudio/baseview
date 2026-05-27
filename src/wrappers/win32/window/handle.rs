use std::marker::PhantomData;
use std::ptr::NonNull;
use windows_core::{Error, Result, HRESULT};
use windows_sys::Win32::Foundation::{SetLastError, HWND};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA,
};

/// A simple wrapper around a HWND.
///
/// This type guarantees the HWND is safe to use, but not that it remains valid. (i.e. functions using
/// a handle from this type might still return an "invalid handle" error).
///
/// The role of this type is to help safely encapsulating most of the unsafe Win32 HWND APIs.
#[derive(Copy, Clone)]
pub struct HWnd<'a>(HWND, PhantomData<&'a ()>);

impl HWnd<'_> {
    pub unsafe fn from_raw(hwnd: HWND) -> Self {
        Self(hwnd, PhantomData)
    }

    pub fn as_raw(&self) -> HWND {
        self.0
    }

    pub fn get_userdata_ptr<T>(&self) -> Option<NonNull<T>> {
        let ptr = unsafe { GetWindowLongPtrW(self.0, GWLP_USERDATA) };
        NonNull::new(ptr as *mut T)
    }

    pub fn set_userdata_ptr<T>(&self, data: *const T) -> Result<()> {
        // SAFETY: This function is always safe to call
        unsafe { SetLastError(0) };
        // SAFETY: This type guarantees the HWND is safe to use.
        let previous = unsafe { SetWindowLongPtrW(self.0, GWLP_USERDATA, data as isize) };
        if previous != 0 {
            return Ok(());
        }

        // We can't know if a return value of 0 is indicative of an error, or if it's just because the
        // previous value was 0. So we check GetLastError instead (called by Error::from_win32).
        let error = Error::from_win32();
        if error.code() == HRESULT(0) {
            return Ok(());
        }

        Err(error)
    }
}
