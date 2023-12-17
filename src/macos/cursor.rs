use cocoa::base::id;
use objc::{runtime::Sel, msg_send, sel, sel_impl, class};

use crate::MouseCursor;

#[derive(Debug)]
pub enum Cursor {
    Native(&'static str),
    Undocumented(&'static str),
}

impl From<MouseCursor> for Cursor {
    fn from(cursor: MouseCursor) -> Self {
        match cursor {
            MouseCursor::Default => Cursor::Native("arrowCursor"),
            MouseCursor::Pointer => Cursor::Native("pointingHandCursor"),
            MouseCursor::Hand => Cursor::Native("openHandCursor"),
            MouseCursor::HandGrabbing => Cursor::Native("closedHandCursor"),
            MouseCursor::Text => Cursor::Native("IBeamCursor"),
            MouseCursor::VerticalText => Cursor::Native("IBeamCursorForVerticalLayout"),
            MouseCursor::Copy => Cursor::Native("dragCopyCursor"),
            MouseCursor::Alias => Cursor::Native("dragLinkCursor"),
            MouseCursor::NotAllowed | MouseCursor::PtrNotAllowed => {
                Cursor::Native("operationNotAllowedCursor")
            }
            // MouseCursor:: => Cursor::Native("contextualMenuCursor"),
            MouseCursor::Crosshair => Cursor::Native("crosshairCursor"),
            MouseCursor::EResize => Cursor::Native("resizeRightCursor"),
            MouseCursor::NResize => Cursor::Native("resizeUpCursor"),
            MouseCursor::WResize => Cursor::Native("resizeLeftCursor"),
            MouseCursor::SResize => Cursor::Native("resizeDownCursor"),
            MouseCursor::EwResize | MouseCursor::ColResize => Cursor::Native("resizeLeftRightCursor"),
            MouseCursor::NsResize | MouseCursor::RowResize => Cursor::Native("resizeUpDownCursor"),

            MouseCursor::Help => Cursor::Undocumented("_helpCursor"),
            MouseCursor::ZoomIn => Cursor::Undocumented("_zoomInCursor"),
            MouseCursor::ZoomOut => Cursor::Undocumented("_zoomOutCursor"),
            MouseCursor::NeResize => Cursor::Undocumented("_windowResizeNorthEastCursor"),
            MouseCursor::NwResize => Cursor::Undocumented("_windowResizeNorthWestCursor"),
            MouseCursor::SeResize => Cursor::Undocumented("_windowResizeSouthEastCursor"),
            MouseCursor::SwResize => Cursor::Undocumented("_windowResizeSouthWestCursor"),
            MouseCursor::NeswResize => Cursor::Undocumented("_windowResizeNorthEastSouthWestCursor"),
            MouseCursor::NwseResize => Cursor::Undocumented("_windowResizeNorthWestSouthEastCursor"),

            MouseCursor::Working | MouseCursor::PtrWorking => {
                Cursor::Undocumented("busyButClickableCursor")
            }

            _ => Cursor::Native("arrowCursor"),

            // MouseCursor::Hidden => todo!(),
            // MouseCursor::Move => todo!(),
            // MouseCursor::AllScroll => todo!(),
            // MouseCursor::Cell => todo!(),
        }
    }
}

impl Cursor {
    pub unsafe fn load(&self) -> id {
        match self {
            Cursor::Native(cursor_name) => {
                let sel = Sel::register(cursor_name);
                msg_send![class!(NSCursor), performSelector: sel]
            }
            Cursor::Undocumented(cursor_name) => {
                let class = class!(NSCursor);
                let sel = Sel::register(cursor_name);
                let sel = if msg_send![class, respondsToSelector: sel] {
                    sel
                } else {
                    sel!(arrowCursor)
                };
                msg_send![class, performSelector: sel]
            }
        }
    }
}