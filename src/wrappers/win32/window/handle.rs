use crate::wrappers::win32::Rect;
use crate::PhySize;
use std::marker::PhantomData;
use std::ptr::{null_mut, NonNull};
use windows::Win32::System::Ole::IDropTarget;
use windows_core::{Error, Interface, InterfaceRef, Result, HRESULT};
use windows_sys::Win32::Foundation::{SetLastError, HWND, S_OK};
use windows_sys::Win32::System::Ole::RegisterDragDrop;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWLP_USERDATA, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER,
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

    pub fn get_dpi(&self) -> Result<u32> {
        // SAFETY: This type guarantees the HWND is safe to use.
        match unsafe { GetDpiForWindow(self.0) } {
            0 => Err(Error::from_win32()),
            dpi => Ok(dpi),
        }
    }

    pub fn register_drag_drop(&self, drop_target: InterfaceRef<IDropTarget>) -> Result<()> {
        // SAFETY: This type guarantees the HWND is safe to use,
        // and the interface pointer comes from a valid InterfaceRef.
        let result = unsafe { RegisterDragDrop(self.0, drop_target.as_raw()) };

        match result {
            S_OK => Ok(()),
            e => Err(Error::new(HRESULT(e), "RegisterDragDrop failed")),
        }
    }

    pub fn resize_nc_and_activate(&self, size: PhySize) -> Result<()> {
        let result = unsafe {
            SetWindowPos(
                self.0,
                null_mut(), // Ignored by SWP_NOZORDER
                0,          // Ignored by SWP_NOMOVE
                0,          // Ignored by SWP_NOMOVE
                size.width.try_into().unwrap_or(i32::MAX),
                size.height.try_into().unwrap_or(i32::MAX),
                SWP_NOZORDER | SWP_NOMOVE,
            )
        };

        if result == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }

    pub fn resize_and_activate(&self, client_size: PhySize, style: u32) -> Result<()> {
        let rect = Rect::from(client_size).client_area_to_nc_area(style)?;

        self.resize_nc_and_activate(rect.size())
    }

    pub fn set_nc_rect(&self, nc_rect: Rect) -> Result<()> {
        let size = nc_rect.size();

        let result = unsafe {
            SetWindowPos(
                self.0,
                null_mut(), // Ignored by SWP_NOZORDER
                nc_rect.0.left,
                nc_rect.0.top,
                size.width.try_into().unwrap_or(i32::MAX),
                size.height.try_into().unwrap_or(i32::MAX),
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };

        if result == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }
}
