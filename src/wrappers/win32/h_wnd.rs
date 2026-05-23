use std::ptr::NonNull;
use windows_core::{Error, Result, HRESULT};
use windows_sys::Win32::Foundation::{SetLastError, HWND};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowLongW, SetWindowLongPtrW, ShowWindow, GWLP_USERDATA, SW_SHOWNORMAL,
    WINDOW_LONG_PTR_INDEX,
};

#[repr(transparent)]
pub struct HWnd(HWND);

impl HWnd {
    pub unsafe fn from_ref(hwnd: &HWND) -> &Self {
        // SAFETY: HWnd is repr(transparent)
        unsafe { &*(hwnd as *const HWND as *const HWnd) }
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
        // SAFETY: This type guarantees the HWND is still valid.
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
