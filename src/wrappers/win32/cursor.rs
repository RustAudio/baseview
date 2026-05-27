use crate::MouseCursor;
use std::ffi::c_void;
use std::ptr::{null_mut, NonNull};
use windows_core::{Error, Result};
use windows_sys::Win32::UI::WindowsAndMessaging::{LoadCursorW, SetCursor};
use windows_sys::{
    core::PCWSTR,
    Win32::UI::WindowsAndMessaging::{
        IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL,
        IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT,
    },
};

#[derive(Copy, Clone)]
pub struct SystemCursor(NonNull<c_void>);

impl SystemCursor {
    pub fn load(cursor: MouseCursor) -> Result<Self> {
        let cursor_ptr = cursor_to_lpcwstr(cursor);

        // SAFETY: the PCWSTR returned by cursor_to_lpcwstr is always a valid shared cursor ID
        let result = unsafe { LoadCursorW(null_mut(), cursor_ptr) };

        match NonNull::new(result) {
            Some(res) => Ok(Self(res)),
            None => Err(Error::from_win32()),
        }
    }

    pub fn set(&self) {
        // SAFETY: This type guarantees the HCURSOR was returned by a successful call to LoadCursorW
        unsafe { SetCursor(self.0.as_ptr()) };
    }
}

fn cursor_to_lpcwstr(cursor: MouseCursor) -> PCWSTR {
    match cursor {
        MouseCursor::Default => IDC_ARROW,
        MouseCursor::Hand => IDC_HAND,
        MouseCursor::HandGrabbing => IDC_SIZEALL,
        MouseCursor::Help => IDC_HELP,
        // an empty LPCWSTR results in the cursor being hidden
        MouseCursor::Hidden => std::ptr::null(),

        MouseCursor::Text => IDC_IBEAM,
        MouseCursor::VerticalText => IDC_IBEAM,

        MouseCursor::Working => IDC_WAIT,
        MouseCursor::PtrWorking => IDC_APPSTARTING,

        MouseCursor::NotAllowed => IDC_NO,
        MouseCursor::PtrNotAllowed => IDC_NO,

        MouseCursor::ZoomIn => IDC_ARROW,
        MouseCursor::ZoomOut => IDC_ARROW,

        MouseCursor::Alias => IDC_ARROW,
        MouseCursor::Copy => IDC_ARROW,
        MouseCursor::Move => IDC_SIZEALL,
        MouseCursor::AllScroll => IDC_SIZEALL,
        MouseCursor::Cell => IDC_CROSS,
        MouseCursor::Crosshair => IDC_CROSS,

        MouseCursor::EResize => IDC_SIZEWE,
        MouseCursor::NResize => IDC_SIZENS,
        MouseCursor::NeResize => IDC_SIZENESW,
        MouseCursor::NwResize => IDC_SIZENWSE,
        MouseCursor::SResize => IDC_SIZENS,
        MouseCursor::SeResize => IDC_SIZENWSE,
        MouseCursor::SwResize => IDC_SIZENESW,
        MouseCursor::WResize => IDC_SIZEWE,
        MouseCursor::EwResize => IDC_SIZEWE,
        MouseCursor::NsResize => IDC_SIZENS,
        MouseCursor::NwseResize => IDC_SIZENWSE,
        MouseCursor::NeswResize => IDC_SIZENESW,

        MouseCursor::ColResize => IDC_SIZEWE,
        MouseCursor::RowResize => IDC_SIZENS,
    }
}
