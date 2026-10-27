use crate::wrappers::win32::window::{HWnd, WindowData, WindowImpl};
use crate::wrappers::win32::{Dpi, Rect};
use crate::{Event, MouseButton, MouseEvent, ScrollDelta};
use dpi::{PhysicalPosition, PhysicalSize};
use std::num::NonZeroU32;
use std::ptr::NonNull;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub trait WmSizing {
    fn on_size(&self, window: HWnd, new_size: PhysicalSize<u32>) -> Result<(), ()>;
    fn on_dpi_changed(&self, window: HWnd, new_dpi: Dpi, suggested_nc_rect: Rect)
        -> Result<(), ()>;
    fn get_min_max_size(
        &self, window: HWnd,
    ) -> (Option<PhysicalSize<i32>>, Option<PhysicalSize<i32>>);
}

pub trait WmMouse {
    fn on_move(&self, window: HWnd, position: PhysicalPosition<i16>, wparam: WPARAM);
    fn on_leave(&self, window: HWnd);
    fn on_wheel(&self, window: HWnd, delta: ScrollDelta, wparam: WPARAM);
    fn on_capture_changed(&self, window: HWnd, window_gaining_capture: Option<HWnd>);
    fn on_button(&self, window: HWnd, button: MouseButton, down: bool, wparam: WPARAM);
}

pub unsafe fn handle_message<W: WindowImpl>(
    data: &WindowData<W>, window: HWnd, message_code: u32, wparam: WPARAM, lparam: LPARAM,
) -> Option<LRESULT> {
    let inner = data.inner()?;

    match message_code {
        // Sizing
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

        // Mouse
        WM_MOUSEMOVE => {
            inner.on_move(
                window,
                PhysicalPosition {
                    x: (lparam & 0xFFFF) as i16,
                    y: ((lparam >> 16) & 0xFFFF) as i16,
                },
                wparam,
            );

            Some(0)
        }

        WM_MOUSELEAVE => {
            inner.on_leave(window);
            Some(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let value = (wparam >> 16) as i16 as i32;
            let value = value as f32 / WHEEL_DELTA as f32;

            let delta = if message_code == WM_MOUSEWHEEL {
                ScrollDelta::Lines { x: 0.0, y: value }
            } else {
                ScrollDelta::Lines { x: value, y: 0.0 }
            };

            inner.on_wheel(window, delta, wparam);

            Some(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_RBUTTONDOWN
        | WM_RBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            fn get_xbutton_wparam(wparam: WPARAM) -> u16 {
                hi_word(wparam)
            }

            const XBUTTON1: u16 = 0x1;
            const XBUTTON2: u16 = 0x2;

            let button = match message_code {
                WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
                WM_MBUTTONDOWN | WM_MBUTTONUP => MouseButton::Middle,
                WM_RBUTTONDOWN | WM_RBUTTONUP => MouseButton::Right,
                WM_XBUTTONDOWN | WM_XBUTTONUP => match get_xbutton_wparam(wparam) {
                    XBUTTON1 => MouseButton::Back,
                    XBUTTON2 => MouseButton::Forward,
                    _ => return None,
                },
                _ => return None,
            };

            let down = match message_code {
                WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN | WM_XBUTTONDOWN => true,
                WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP | WM_XBUTTONUP => false,
                _ => unreachable!(),
            };

            inner.on_button(window, button, down, wparam);

            Some(0)
        }
        WM_CAPTURECHANGED => {
            let window_gaining_capture =
                NonNull::new(lparam as _).map(|p| unsafe { HWnd::from_raw(p) });

            inner.on_capture_changed(window, window_gaining_capture);

            Some(0)
        }
        _ => None,
    }
}

fn hi_word(wparam: WPARAM) -> u16 {
    ((wparam >> 16) & 0xffff) as u16
}

fn lo_word(lparam: LPARAM) -> u16 {
    (lparam & 0xffff) as u16
}
