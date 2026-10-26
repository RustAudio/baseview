use crate::wrappers::win32::window::{HWnd, WindowData, WindowImpl};
use crate::wrappers::win32::{Dpi, Rect};
use dpi::PhysicalSize;
use std::num::NonZeroU32;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub trait WmSizing {
    fn on_size(&self, window: HWnd, new_size: PhysicalSize<u32>) -> Result<(), ()>;
    fn on_dpi_changed(&self, window: HWnd, new_dpi: Dpi, suggested_nc_rect: Rect)
        -> Result<(), ()>;
    fn get_min_max_size(
        &self, window: HWnd,
    ) -> (Option<PhysicalSize<i32>>, Option<PhysicalSize<i32>>);
}

pub unsafe fn handle_message<W: WindowImpl>(
    data: &WindowData<W>, window: HWnd, message_code: u32, wparam: WPARAM, lparam: LPARAM,
) -> Option<LRESULT> {
    let inner = data.inner()?;

    match message_code {
        WM_SIZE => {
            let width = (lparam & 0xFFFF) as u16 as u32;
            let height = ((lparam >> 16) & 0xFFFF) as u16 as u32;
            let new_size = PhysicalSize { width, height };

            match inner.on_size(window, new_size) {
                Ok(()) => None,
                Err(()) => Some(-1),
            }
        }
        WM_DPICHANGED => {
            let suggested_nc_rect = Rect((lparam as *const RECT).read());
            let Some(dpi) = NonZeroU32::new((wparam & 0xFFFF) as u16 as u32) else {
                return Some(-1);
            };
            let dpi = Dpi(dpi);

            match inner.on_dpi_changed(window, dpi, suggested_nc_rect) {
                Ok(()) => None,
                Err(()) => Some(-1),
            }
        }
        WM_GETMINMAXINFO => {
            let (min_size, max_size) = inner.get_min_max_size(window);

            if matches!((min_size, max_size), (None, None)) {
                return None;
            }

            let info = lparam as *mut MINMAXINFO;

            if let Some(size) = min_size {
                (&raw mut (*info).ptMinTrackSize).write(POINT { x: size.width, y: size.height });
            }

            if let Some(size) = max_size {
                (&raw mut (*info).ptMaxTrackSize).write(POINT { x: size.width, y: size.height });
            }

            Some(0)
        }

        _ => None,
    }
}
