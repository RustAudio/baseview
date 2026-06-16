use crate::wrappers::win32::Rect;
use windows_sys::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub enum WindowMessage {
    MouseLeave,
    MouseMove { x: i16, y: i16 },
    Size { client_width: u16, client_height: u16 },
    DpiChanged { dpi_x: u16, _dpi_y: u16, suggested_rect: Option<Rect> },
}

impl WindowMessage {
    pub unsafe fn parse(message: u32, w_param: WPARAM, l_param: LPARAM) -> Option<Self> {
        Some(match message {
            WM_MOUSELEAVE => Self::MouseLeave,
            WM_MOUSEMOVE => {
                Self::MouseMove { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_SIZE => Self::Size {
                client_width: l_loword_unsigned(l_param),
                client_height: l_hiword_unsigned(l_param),
            },
            WM_DPICHANGED => Self::DpiChanged {
                dpi_x: w_loword_unsigned(w_param),
                _dpi_y: w_hiword_unsigned(w_param),
                suggested_rect: if (l_param as *const RECT).is_null() {
                    None
                } else {
                    let rect = unsafe { (l_param as *const RECT).read() };

                    Some(Rect(rect))
                },
            },
            WM_LBUTTONDOWN => {
                Self::LButtonDown { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_LBUTTONUP => {
                Self::LButtonUp { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_MBUTTONDOWN => {
                Self::MButtonDown { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_MBUTTONUP => {
                Self::MButtonUp { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_RBUTTONDOWN => {
                Self::RButtonDown { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_RBUTTONUP => {
                Self::RButtonUp { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_XBUTTONDOWN => {
                Self::XButtonDown { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            WM_XBUTTONUP => {
                Self::XButtonUp { x: l_loword_signed(l_param), y: l_hiword_signed(l_param) }
            }
            _ => return None,
        })
    }
}

#[inline]
const fn w_loword_unsigned(w_param: WPARAM) -> u16 {
    (w_param & 0xFFFF) as u16
}

#[inline]
const fn w_hiword_unsigned(w_param: WPARAM) -> u16 {
    ((w_param >> 16) & 0xFFFF) as u16
}

#[inline]
const fn l_loword_unsigned(w_param: LPARAM) -> u16 {
    (w_param & 0xFFFF) as u16
}

#[inline]
const fn l_hiword_unsigned(w_param: LPARAM) -> u16 {
    ((w_param >> 16) & 0xFFFF) as u16
}

#[inline]
#[allow(dead_code)] // May be used in the future
const fn w_loword_signed(w_param: WPARAM) -> i16 {
    (w_param & 0xFFFF) as i16
}

#[inline]
#[allow(dead_code)] // May be used in the future
const fn w_hiword_signed(w_param: WPARAM) -> i16 {
    ((w_param >> 16) & 0xFFFF) as i16
}

#[inline]
const fn l_loword_signed(w_param: LPARAM) -> i16 {
    (w_param & 0xFFFF) as i16
}

#[inline]
const fn l_hiword_signed(w_param: LPARAM) -> i16 {
    ((w_param >> 16) & 0xFFFF) as i16
}
