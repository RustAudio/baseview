use crate::wrappers::win32::style::WindowStyle;
use crate::wrappers::win32::{Dpi, DpiAwarenessContext, Rect};
use crate::PhySize;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::ptr::{null_mut, NonNull};
use windows::Win32::System::Ole::IDropTarget;
use windows_core::{Error, Interface, InterfaceRef, Result, HRESULT};
use windows_sys::Win32::Foundation::{SetLastError, HWND, S_OK};
use windows_sys::Win32::System::Ole::{RegisterDragDrop, RevokeDragDrop};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetWindowLongPtrW, GetWindowLongW, SetTimer, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
    SW_SHOW, WINDOW_LONG_PTR_INDEX,
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

    pub fn get_long(&self, index: WINDOW_LONG_PTR_INDEX) -> Result<i32> {
        // SAFETY: This function is always safe to call
        unsafe { SetLastError(0) };
        // SAFETY: This type guarantees the HWND is still valid.
        let result = unsafe { GetWindowLongW(self.0, index) };
        if result != 0 {
            return Ok(result);
        }

        // We can't know if a return value of 0 is indicative of an error, or if it's just because the
        // value was actually 0. So we check GetLastError instead (called by Error::from_win32).
        let error = Error::from_win32();
        if error.code() == HRESULT(0) {
            return Ok(result);
        }

        Err(error)
    }

    pub fn get_style(&self) -> Result<WindowStyle> {
        Ok(WindowStyle {
            style: self.get_long(GWL_STYLE)? as _,
            style_ex: self.get_long(GWL_EXSTYLE)? as _,
        })
    }

    pub fn get_dpi(&self) -> Result<Dpi> {
        // SAFETY: This type guarantees the HWND is safe to use.
        match unsafe { GetDpiForWindow(self.0) } {
            0 => Err(Error::from_win32()),
            dpi => Ok(Dpi(dpi)),
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

    pub fn revoke_drag_drop(&self) -> Result<()> {
        let result = unsafe { RevokeDragDrop(self.0) };

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

    pub fn resize_and_activate(&self, client_size: PhySize, window_dpi: Dpi) -> Result<()> {
        let dpi_ctx = DpiAwarenessContext::new()?;
        let style = self.get_style()?;

        let rect = Rect::from(client_size);
        let rect = dpi_ctx.client_area_to_nc_area(rect, style, window_dpi)?;

        self.resize_nc_and_activate(rect.size())
    }

    /// Returns true if the window was previously visible, false otherwise
    pub fn show_and_activate(&self) -> bool {
        let result = unsafe { ShowWindow(self.0, SW_SHOW) };

        result != 0
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

    pub fn set_timer(&self, timer_id: NonZeroUsize, elapse: u32) -> Result<()> {
        let result = unsafe { SetTimer(self.0, timer_id.get(), elapse, None) };

        if result == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }

    pub fn set_focus(&self) -> Result<()> {
        let previous = unsafe { SetFocus(self.0) };
        if !previous.is_null() {
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

    pub fn destroy(&self) -> Result<()> {
        let result = unsafe { DestroyWindow(self.0) };

        if result == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }

    pub fn get_focused_window() -> HWND {
        // SAFETY: this is always safe to call
        unsafe { GetFocus() }
    }

    pub fn set_capture(&self) {
        // SAFETY: This type guarantees the HWND is safe to use.
        unsafe { SetCapture(self.0) };
    }

    pub fn release_capture() {
        // SAFETY: this is always safe to call
        unsafe { ReleaseCapture() };
    }

    pub fn start_cursor_leave_tracking(&self) -> Result<()> {
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            dwHoverTime: 0,
            hwndTrack: self.0,
        };

        // SAFETY: eventtrack pointer comes from a reference, and the struct it points to is filled
        // correctly
        match unsafe { TrackMouseEvent(&mut track) } {
            0 => Err(Error::from_win32()),
            _ => Ok(()),
        }
    }
}
