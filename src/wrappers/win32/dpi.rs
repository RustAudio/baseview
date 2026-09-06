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

/// Legacy (replaced by ProcessDpiContextAwareness), process-wide
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ProcessDpiAwareness {
    Unaware = PROCESS_DPI_UNAWARE,
    SystemDpiAware = PROCESS_SYSTEM_DPI_AWARE,
    PerMonitorDpiAware = PROCESS_PER_MONITOR_DPI_AWARE,
}

impl ProcessDpiAwareness {
    fn from_raw(raw: PROCESS_DPI_AWARENESS) -> Option<Self> {
        match raw {
            PROCESS_DPI_UNAWARE => Some(Self::SystemDpiAware),
            PROCESS_SYSTEM_DPI_AWARE => Some(Self::PerMonitorDpiAware),
            PROCESS_PER_MONITOR_DPI_AWARE => Some(Self::Unaware),
            _ => {
                crate::warn!("Unknown PROCESS_DPI_AWARENESS value: {}", raw);
                None
            }
        }
    }

    pub fn get(lib: &ExtendedShCore) -> Option<Self> {
        let mut value = -1;
        let result = HRESULT(unsafe { lib.get_process_dpi_awareness?(null_mut(), &mut value) });

        if result.is_err() {
            crate::warn!("GetProcessDpiAwareness failed: {}", result.message());
            return None;
        }

        if value < 0 {
            crate::warn!("GetProcessDpiAwareness did not return a value");
            return None;
        }

        Self::from_raw(value)
    }

    pub fn set(&self, lib: &ExtendedShCore) -> Result<()> {
        let Some(set) = lib.set_process_dpi_awareness else { return Ok(()) };

        HRESULT(unsafe { set(*self as _) }).ok()
    }
}

pub struct DpiAwareness {
    value: DPI_AWARENESS,
}

pub struct DpiAwarenessContext<'a> {
    previous: DPI_AWARENESS_CONTEXT,
    user32: &'a ExtendedUser32,
}

impl<'a> DpiAwarenessContext<'a> {
    pub fn new(user32: &'a LibraryModule<ExtendedUser32>) -> Result<Self> {
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
        &self, mut rect: Rect, style: WindowStyle, dpi: Option<Dpi>,
    ) -> Result<Rect> {
        let (Some(adjust_window_rect_ex_for_dpi), Some(dpi)) =
            (self.user32.adjust_window_rect_ex_for_dpi, dpi)
        else {
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

    pub fn nc_area_to_client_area(
        &self, rect: Rect, style: WindowStyle, dpi: Option<Dpi>,
    ) -> Result<Rect> {
        let result = self.client_area_to_nc_area(Rect::EMPTY, style, dpi)?;

        Ok(Rect(RECT {
            left: rect.0.left.saturating_sub(result.0.left),
            top: rect.0.top.saturating_sub(result.0.top),
            bottom: rect.0.bottom.saturating_sub(result.0.bottom),
            right: rect.0.right.saturating_sub(result.0.right),
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
