use super::*;
use crate::platform::DpiScalingStrategy;
use crate::wrappers::win32::user32::ExtendedUser32;
use crate::wrappers::win32::DpiAwarenessContextType::*;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::ptr::NonNull;
use windows_core::{Error, Result};
use windows_sys::Win32::Foundation::{FALSE, RECT, TRUE};
use windows_sys::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGPIXELSX};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{AdjustWindowRectEx, USER_DEFAULT_SCREEN_DPI};

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Dpi(pub NonZeroU32);

impl Dpi {
    pub fn scale_factor(&self) -> f64 {
        self.0.get() as f64 / USER_DEFAULT_SCREEN_DPI as f64
    }

    /// Windows 10, version 1607.
    pub fn get_system(user32: &ExtendedUser32) -> Option<Self> {
        if let Some(get_dpi_for_system) = user32.get_dpi_for_system {
            Some(Self(NonZeroU32::new(unsafe { get_dpi_for_system() })?))
            // This is unlikely to be present if the above isn't, but it's worth a try
        } else if let Some(get_system_dpi_for_process) = user32.get_system_dpi_for_process {
            Some(Self(NonZeroU32::new(unsafe { get_system_dpi_for_process(null_mut()) })?))
        } else {
            Self::get_from_device_caps()
        }
    }

    fn get_from_device_caps() -> Option<Self> {
        struct DisplayDC(NonNull<c_void>);

        impl DisplayDC {
            fn get() -> Option<Self> {
                let result = unsafe { GetDC(null_mut()) };
                NonNull::new(result).map(Self)
            }

            fn dpi(&self) -> Option<Dpi> {
                let result = unsafe { GetDeviceCaps(self.0.as_ptr(), LOGPIXELSX as _) };
                let result: u32 = result.try_into().ok()?;
                NonZeroU32::new(result).map(Dpi)
            }
        }

        impl Drop for DisplayDC {
            fn drop(&mut self) {
                let _ = unsafe { ReleaseDC(null_mut(), self.0.as_ptr()) };
            }
        }

        DisplayDC::get()?.dpi()
    }

    pub const USER_DEFAULT: Self = Self(match NonZeroU32::new(USER_DEFAULT_SCREEN_DPI) {
        None => unreachable!(),
        Some(dpi) => dpi,
    });
}

impl Default for Dpi {
    fn default() -> Self {
        Self::USER_DEFAULT
    }
}

/// Win8 Legacy (replaced by DpiAwarenessContext in Win10), process-wide.
#[repr(i32)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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

/// Windows 10, version 1607.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DpiAwarenessContextType {
    Unaware,
    UnawareGDIScaled,
    SystemDpiAware,
    PerMonitorDpiAware,
    /// Windows 10, version 1703.
    PerMonitorDpiAwareV2,
}

impl DpiAwarenessContextType {
    // Sorted by order of goodness
    const ALL: [Self; 5] =
        [PerMonitorDpiAwareV2, PerMonitorDpiAware, SystemDpiAware, Unaware, UnawareGDIScaled];

    /// Windows 10, version 1607.
    pub fn best_supported(user32: &ExtendedUser32) -> Option<Self> {
        for awareness_type in Self::ALL {
            if DpiAwarenessContext::from(awareness_type).is_valid(user32)? {
                return Some(awareness_type);
            }
        }

        None
    }
}

#[derive(Copy, Clone)]
pub struct DpiAwarenessContext {
    inner: NonNull<c_void>,
}

impl DpiAwarenessContext {
    pub fn from_raw(raw: NonNull<c_void>) -> Self {
        Self { inner: raw }
    }

    /// Windows 10, version 1607.
    pub fn is_valid(&self, user32: &ExtendedUser32) -> Option<bool> {
        Some(unsafe { user32.is_valid_dpi_awareness_context?(self.inner.as_ptr()) } == TRUE)
    }

    /// Windows 10, version 1607.
    pub fn set_thread(&self, user32: &ExtendedUser32) -> Option<Result<DpiAwarenessContext>> {
        let previous = unsafe {
            user32.set_thread_dpi_awareness_context?(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };

        let Some(inner) = NonNull::new(previous) else { return Some(Err(Error::from_thread())) };

        Some(Ok(Self { inner }))
    }

    pub fn set_process(&self, user32: &ExtendedUser32) -> Option<Result<()>> {
        let result = unsafe {
            user32.set_process_dpi_awareness_context?(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };

        if result == FALSE {
            return Some(Err(Error::from_thread()));
        }

        Some(Ok(()))
    }

    pub fn get_from_process(user32: &ExtendedUser32) -> Option<Self> {
        let context = unsafe { user32.get_dpi_awareness_context_for_process?(null_mut()) };

        NonNull::new(context).map(Self::from_raw)
    }

    /// Windows 10, version 1803.
    pub fn dpi(&self, user32: &ExtendedUser32) -> Option<Dpi> {
        let result = unsafe { user32.get_dpi_from_dpi_awareness_context?(self.inner.as_ptr()) };

        Some(Dpi(NonZeroU32::new(result)?))
    }

    /// Windows 10, version 1607.
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

    /// Returns None if type is unknown.
    ///
    /// Windows 10, version 1607.
    pub(crate) fn get_type(&self, user32: &ExtendedUser32) -> Option<DpiAwarenessContextType> {
        for dpi_type in DpiAwarenessContextType::ALL {
            let context = DpiAwarenessContext::from(dpi_type);
            if context.is_valid(user32)? && self.equals(context, user32)? {
                return Some(dpi_type);
            }
        }

        None
    }
}

impl From<DpiAwarenessContextType> for DpiAwarenessContext {
    fn from(value: DpiAwarenessContextType) -> Self {
        use DpiAwarenessContextType::*;

        let inner = match value {
            Unaware => DPI_AWARENESS_CONTEXT_UNAWARE,
            UnawareGDIScaled => DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED,
            SystemDpiAware => DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
            PerMonitorDpiAware => DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
            PerMonitorDpiAwareV2 => DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };

        let Some(inner) = NonNull::new(inner) else { unreachable!() };

        Self { inner }
    }
}

pub struct DpiAwarenessGuard<'a> {
    inner: Option<(DpiAwarenessContext, &'a ExtendedUser32)>,
}

impl<'a> DpiAwarenessGuard<'a> {
    pub fn new(user32: &'a ExtendedUser32, strategy: DpiScalingStrategy) -> Result<Self> {
        let Some(new_context) = strategy.thread_dpi_awareness_context else {
            return Ok(Self { inner: None });
        };

        match new_context.set_thread(user32) {
            None => Ok(Self { inner: None }),
            Some(Err(e)) => Err(e),
            Some(Ok(previous)) => Ok(Self { inner: Some((previous, user32)) }),
        }
    }

    pub fn with_guard_optional<T>(
        user32: &'a ExtendedUser32, strategy: DpiScalingStrategy, handler: impl FnOnce() -> T,
    ) -> T {
        let guard = match DpiAwarenessGuard::new(user32, strategy) {
            Ok(guard) => Some(guard),
            Err(e) => {
                crate::warn!("Could not set up thread DPIAwarenessContext: {}", e);
                None
            }
        };

        let result = handler();

        drop(guard);

        result
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
                adjust_window_rect_ex_for_dpi(
                    &mut rect.0,
                    style.style,
                    0,
                    style.style_ex,
                    dpi.0.get(),
                )
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
