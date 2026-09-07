use super::*;
use crate::wrappers::win32::user32::ExtendedUser32;
use std::ffi::c_void;
use std::ptr::NonNull;
use windows_core::{Error, Result};
use windows_sys::Win32::Foundation::{RECT, TRUE};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{AdjustWindowRectEx, USER_DEFAULT_SCREEN_DPI};

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Dpi(pub u32);

impl Dpi {
    pub fn scale_factor(&self) -> f64 {
        self.0 as f64 / USER_DEFAULT_SCREEN_DPI as f64
    }

    /// Windows 10, version 1607
    pub fn get_system(user32: &ExtendedUser32) -> Option<Self> {
        if let Some(get_dpi_for_system) = user32.get_dpi_for_system {
            Some(Self(unsafe { get_dpi_for_system() }))
        } else if let Some(get_system_dpi_for_process) = user32.get_system_dpi_for_process {
            Some(Self(unsafe { get_system_dpi_for_process(null_mut()) }))
        } else {
            None
        }
    }
}

impl Default for Dpi {
    fn default() -> Self {
        Self(USER_DEFAULT_SCREEN_DPI)
    }
}

/// Win8 Legacy (replaced by DpiAwarenessContext in Win10), process-wide
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DpiAwarenessContextType {
    Unaware,
    SystemDpiAware,
    PerMonitorDpiAware,
    PerMonitorDpiAwareV2,
}

#[derive(Copy, Clone)]
pub struct DpiAwarenessContext {
    inner: NonNull<c_void>,
}

impl DpiAwarenessContext {
    /// Windows 10, version 1607
    pub fn is_valid(&self, user32: &ExtendedUser32) -> Option<bool> {
        Some(unsafe { user32.is_valid_dpi_awareness_context?(self.inner.as_ptr()) } == TRUE)
    }

    /// Windows 10, version 1607
    pub fn set_thread(&self, user32: &ExtendedUser32) -> Option<Result<DpiAwarenessContext>> {
        let previous = unsafe {
            user32.set_thread_dpi_awareness_context?(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };

        let Some(inner) = NonNull::new(previous) else { return Some(Err(Error::from_thread())) };

        Some(Ok(Self { inner }))
    }

    /// Windows 10, version 1803
    pub fn dpi(&self, user32: &ExtendedUser32) -> Option<Dpi> {
        todo!()
    }

    /// Windows 10, version 1607
    pub fn equals(
        &self, other: impl Into<DpiAwarenessContext>, user32: &ExtendedUser32,
    ) -> Option<bool> {
        Some(
            unsafe {
                user32.are_dpi_awareness_contexts_equal?(
                    self.inner.as_ptr(),
                    other.into().inner.as_ptr(),
                )
            } == TRUE,
        )
    }
}

impl From<DpiAwarenessContextType> for DpiAwarenessContext {
    fn from(value: DpiAwarenessContextType) -> Self {
        let inner = match value {
            DpiAwarenessContextType::Unaware => DPI_AWARENESS_CONTEXT_UNAWARE,
            DpiAwarenessContextType::SystemDpiAware => DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
            DpiAwarenessContextType::PerMonitorDpiAware => DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
            DpiAwarenessContextType::PerMonitorDpiAwareV2 => {
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
            }
        };

        let Some(inner) = NonNull::new(inner) else { unreachable!() };

        Self { inner }
    }
}

pub struct DpiAwareness {
    value: DPI_AWARENESS,
}

pub struct DpiAwarenessGuard<'a> {
    inner: Option<(DpiAwarenessContext, &'a ExtendedUser32)>,
}

impl<'a> DpiAwarenessGuard<'a> {
    pub fn new(user32: &'a ExtendedUser32) -> Result<Self> {
        let Some(set_thread_dpi_awareness_context) = user32.set_thread_dpi_awareness_context else {
            return Ok(Self { inner: None });
        };

        let previous =
            unsafe { set_thread_dpi_awareness_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

        let Some(previous) = NonNull::new(previous) else { return Err(Error::from_thread()) };

        Ok(DpiAwarenessGuard { inner: Some((DpiAwarenessContext { inner: previous }, user32)) })
    }

    pub fn client_area_to_nc_area(
        &self, mut rect: Rect, style: WindowStyle, dpi: Option<Dpi>,
    ) -> Result<Rect> {
        let result = if let (
            Some((
                _,
                ExtendedUser32 {
                    adjust_window_rect_ex_for_dpi: Some(adjust_window_rect_ex_for_dpi),
                    ..
                },
            )),
            Some(dpi),
        ) = (self.inner, dpi)
        {
            // adjust_window_rect_ex_for_dpi takes the current DPI awareness context in consideration.
            // Therefore, this method taking &self enforces that the DPI aware context is correct.
            unsafe {
                adjust_window_rect_ex_for_dpi(&mut rect.0, style.style, 0, style.style_ex, dpi.0)
            }
        } else {
            unsafe { AdjustWindowRectEx(&mut rect.0, style.style, 0, style.style_ex) }
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

impl Drop for DpiAwarenessGuard<'_> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner {
            let _ = inner.0.set_thread(inner.1);
        }
    }
}
