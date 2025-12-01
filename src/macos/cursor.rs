use crate::MouseCursor;
use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};

pub fn mouse_cursor_to_nscursor(cursor: MouseCursor) -> id {
    unsafe {
        let nscursor_class = class!(NSCursor);
        match cursor {
            MouseCursor::Default => msg_send![nscursor_class, arrowCursor],
            MouseCursor::Hand => msg_send![nscursor_class, pointingHandCursor],
            MouseCursor::HandGrabbing => msg_send![nscursor_class, closedHandCursor],
            MouseCursor::Help => msg_send![nscursor_class, arrowCursor], // No help cursor
            MouseCursor::Hidden => {
                // Return a null cursor for hidden - will be handled specially
                std::ptr::null_mut()
            }
            MouseCursor::Text => msg_send![nscursor_class, IBeamCursor],
            MouseCursor::VerticalText => msg_send![nscursor_class, IBeamCursor],
            MouseCursor::Working => msg_send![nscursor_class, arrowCursor],
            MouseCursor::PtrWorking => msg_send![nscursor_class, arrowCursor],
            MouseCursor::NotAllowed => msg_send![nscursor_class, operationNotAllowedCursor],
            MouseCursor::PtrNotAllowed => msg_send![nscursor_class, operationNotAllowedCursor],
            MouseCursor::ZoomIn => msg_send![nscursor_class, arrowCursor],
            MouseCursor::ZoomOut => msg_send![nscursor_class, arrowCursor],
            MouseCursor::Alias => msg_send![nscursor_class, dragLinkCursor],
            MouseCursor::Copy => msg_send![nscursor_class, dragCopyCursor],
            MouseCursor::Move => msg_send![nscursor_class, arrowCursor],
            MouseCursor::AllScroll => msg_send![nscursor_class, arrowCursor],
            MouseCursor::Cell => msg_send![nscursor_class, crosshairCursor],
            MouseCursor::Crosshair => msg_send![nscursor_class, crosshairCursor],
            MouseCursor::EResize => msg_send![nscursor_class, resizeRightCursor],
            MouseCursor::NResize => msg_send![nscursor_class, resizeUpCursor],
            MouseCursor::NeResize => msg_send![nscursor_class, arrowCursor], // No built-in
            MouseCursor::NwResize => msg_send![nscursor_class, arrowCursor], // No built-in
            MouseCursor::SResize => msg_send![nscursor_class, resizeDownCursor],
            MouseCursor::SeResize => msg_send![nscursor_class, arrowCursor], // No built-in
            MouseCursor::SwResize => msg_send![nscursor_class, arrowCursor], // No built-in
            MouseCursor::WResize => msg_send![nscursor_class, resizeLeftCursor],
            MouseCursor::EwResize => msg_send![nscursor_class, resizeLeftRightCursor],
            MouseCursor::NsResize => msg_send![nscursor_class, resizeUpDownCursor],
            MouseCursor::NwseResize => {
                // Use private API for diagonal resize cursor
                msg_send![nscursor_class, _windowResizeNorthWestSouthEastCursor]
            }
            MouseCursor::NeswResize => {
                // Use private API for diagonal resize cursor
                msg_send![nscursor_class, _windowResizeNorthEastSouthWestCursor]
            }
            MouseCursor::ColResize => msg_send![nscursor_class, resizeLeftRightCursor],
            MouseCursor::RowResize => msg_send![nscursor_class, resizeUpDownCursor],
        }
    }
}
