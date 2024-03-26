use crate::PhySize;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use winapi::shared::minwindef::{ATOM, DWORD};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExW, GetDpiForWindow, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER, USER_DEFAULT_SCREEN_DPI, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE,
};

// TODO: handle proper destruction of this window during errors/panics/etc.
pub(crate) struct Win32Window {
    pub handle: HWND,
    style_flags: DWORD,
}

impl Win32Window {
    pub fn create(window_class: ATOM, title: &str, size: PhySize, parent: Option<HWND>) -> Self {
        let mut title: Vec<u16> = OsStr::new(title).encode_wide().collect();
        title.push(0);

        let style_flags = if parent.is_some() {
            WS_CHILD | WS_VISIBLE
        } else {
            WS_POPUPWINDOW
                | WS_CAPTION
                | WS_VISIBLE
                | WS_SIZEBOX
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_CLIPSIBLINGS
        };

        let size = client_size_to_window_size(size, style_flags);

        // TODO: handle errors
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                window_class as _,
                title.as_ptr(),
                style_flags,
                0,
                0,
                size.width as i32,
                size.height as i32,
                parent.unwrap_or(null_mut()),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };

        Win32Window { style_flags, handle: hwnd }
    }

    /// Resizes the window.
    ///
    /// This *will* immediately trigger a WM_SIZE event.
    pub fn resize(&self, size: PhySize) {
        let window_size = client_size_to_window_size(size, self.style_flags);

        unsafe {
            SetWindowPos(
                self.handle,
                null_mut(), // Ignored by SWP_NOZORDER
                0,          // Ignored by SWP_NOMOVE
                0,          // Ignored by SWP_NOMOVE
                window_size.width as i32,
                window_size.height as i32,
                SWP_NOZORDER | SWP_NOMOVE,
            );
        }
    }

    /// Sets both the position and size of the window, according to a given raw RECT.
    ///
    /// This *will* immediately trigger a WM_SIZE event.
    pub fn set_raw_pos(&self, rect: &RECT) {
        unsafe {
            SetWindowPos(
                self.handle,
                null_mut(), // Ignored by SWP_NOZORDER
                rect.left,
                rect.top,
                rect.right.saturating_sub(rect.left),
                rect.bottom.saturating_sub(rect.top),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// Returns current the scale factor of the monitor the window is currently on.
    pub fn current_scale_factor(&self) -> f64 {
        // FIXME: Only works on Windows 10.
        let dpi = unsafe { GetDpiForWindow(self.handle) };
        dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64
    }
}

pub fn client_size_to_window_size(size: PhySize, window_flags: DWORD) -> PhySize {
    let mut rect = RECT {
        left: 0,
        top: 0,
        // In case the provided size overflows an i32, which would imply it is ridiculously large.
        right: i32::try_from(size.width).unwrap(),
        bottom: i32::try_from(size.height).unwrap(),
    };

    // SAFETY: the provided rect pointer is guaranteed to be valid by the mutable reference.
    unsafe { AdjustWindowRectEx(&mut rect, window_flags, 0, 0) };

    // These checks are made just in case AdjustWindowRectEx sends back invalid values.
    // Because this is so unlikely, we can afford to just panic here.
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);

    PhySize { width: u32::try_from(width).unwrap(), height: u32::try_from(height).unwrap() }
}
