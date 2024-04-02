use crate::MouseCursor;
use winapi::{
    shared::ntdef::LPCWSTR,
    um::winuser::{
        IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL,
        IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT,
    },
};

pub fn cursor_to_lpcwstr(cursor: MouseCursor) -> LPCWSTR {
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
