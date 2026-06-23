use super::*;
use crate::wrappers::win32::user32::ExtendedUser32;
use windows_core::{Error, Result};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{AdjustWindowRectEx, USER_DEFAULT_SCREEN_DPI};

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Dpi(pub u32);

impl Dpi {
    pub fn scale_factor(&self) -> f64 {
        self.0 as f64 / USER_DEFAULT_SCREEN_DPI as f64
    }
}

impl Default for Dpi {
    fn default() -> Self {
        Self(USER_DEFAULT_SCREEN_DPI)
    }
}

pub struct DpiAwarenessContext<'a> {
    previous: DPI_AWARENESS_CONTEXT,
    user32: &'a ExtendedUser32,
}

impl<'a> DpiAwarenessContext<'a> {
    pub fn new(user32: &'a ExtendedUser32) -> Result<Self> {
        let Some(set_thread_dpi_awareness_context) = user32.set_thread_dpi_awareness_context else {
            return Ok(Self { previous: null_mut(), user32 });
        };

        let previous =
            unsafe { set_thread_dpi_awareness_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

        if previous.is_null() {
            return Err(Error::from_thread());
        }

        Ok(DpiAwarenessContext { previous, user32 })
    }

    pub fn client_area_to_nc_area(
        &self, mut rect: Rect, style: WindowStyle, dpi: Dpi,
    ) -> Result<Rect> {
        let Some(adjust_window_rect_ex_for_dpi) = self.user32.adjust_window_rect_ex_for_dpi else {
            let result = unsafe { AdjustWindowRectEx(&mut rect.0, style.style, 0, style.style_ex) };

            if result == 0 {
                return Err(Error::from_thread());
            }

            return Ok(rect);
        };

        // adjust_window_rect_ex_for_dpi takes the current DPI awareness context in consideration.
        // Therefore, this method taking &self enforces that the DPI aware context is correct.
        let result = unsafe {
            adjust_window_rect_ex_for_dpi(&mut rect.0, style.style, 0, style.style_ex, dpi.0)
        };

        if result == 0 {
            return Err(Error::from_thread());
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

impl Drop for DpiAwarenessContext<'_> {
    fn drop(&mut self) {
        if let Some(set_thread_dpi_awareness_context) = self.user32.set_thread_dpi_awareness_context
        {
            let _ = unsafe { set_thread_dpi_awareness_context(self.previous) };
        }
    }
}
