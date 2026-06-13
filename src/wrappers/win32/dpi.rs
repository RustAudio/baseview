use super::*;
use windows_core::{Error, Result};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::USER_DEFAULT_SCREEN_DPI;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Dpi(pub u32);

impl Dpi {
    pub const fn scale_factor(&self) -> f64 {
        self.0 as f64 / USER_DEFAULT_SCREEN_DPI as f64
    }
}

impl Default for Dpi {
    fn default() -> Self {
        Self(USER_DEFAULT_SCREEN_DPI)
    }
}

pub struct DpiAwarenessContext {
    previous: DPI_AWARENESS_CONTEXT,
}

impl DpiAwarenessContext {
    pub fn new() -> Result<Self> {
        let mut previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

        if previous.is_null() {
            previous =
                unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE) };
        }

        if previous.is_null() {
            return Err(Error::from_win32());
        }

        Ok(DpiAwarenessContext { previous })
    }

    pub fn client_area_to_nc_area(
        &self, mut rect: Rect, style: WindowStyle, dpi: Dpi,
    ) -> Result<Rect> {
        // AdjustWindowRectExForDpi takes the current DPI awareness context in consideration.
        // Therefore, this method taking &self enforces that the DPI aware context is correct.
        let result =
            unsafe { AdjustWindowRectExForDpi(&mut rect.0, style.style, 0, style.style_ex, dpi.0) };

        if result == 0 {
            return Err(Error::from_win32());
        }

        Ok(rect)
    }

    pub fn nc_area_to_client_area(&self, rect: Rect, style: WindowStyle, dpi: Dpi) -> Result<Rect> {
        let result = self.client_area_to_nc_area(Rect::EMPTY, style, dpi)?;

        Ok(Rect(RECT {
            left: rect.0.left - result.0.left,
            top: rect.0.top - result.0.top,
            bottom: rect.0.bottom - result.0.bottom,
            right: rect.0.right - result.0.right,
        }))
    }
}

impl Drop for DpiAwarenessContext {
    fn drop(&mut self) {
        let _ = unsafe { SetThreadDpiAwarenessContext(self.previous) };
    }
}
