use objc2::__framework_prelude::Retained;
use objc2::runtime::{MessageReceiver, Sel};
use objc2::{msg_send, sel, ClassType};
use objc2_app_kit::NSCursor;

use crate::MouseCursor;

#[derive(Debug)]
pub enum Cursor {
    Native(fn() -> Retained<NSCursor>),
    Undocumented(Sel),
}

impl From<MouseCursor> for Cursor {
    fn from(cursor: MouseCursor) -> Self {
        match cursor {
            MouseCursor::Default => Cursor::Native(NSCursor::arrowCursor),
            MouseCursor::Hand => Cursor::Native(NSCursor::openHandCursor),
            MouseCursor::HandGrabbing => Cursor::Native(NSCursor::closedHandCursor),
            MouseCursor::Text => Cursor::Native(NSCursor::IBeamCursor),
            MouseCursor::VerticalText => Cursor::Native(NSCursor::IBeamCursorForVerticalLayout),
            MouseCursor::Copy => Cursor::Native(NSCursor::dragCopyCursor),
            MouseCursor::Alias => Cursor::Native(NSCursor::dragLinkCursor),
            MouseCursor::NotAllowed | MouseCursor::PtrNotAllowed => {
                Cursor::Native(NSCursor::operationNotAllowedCursor)
            }
            MouseCursor::Crosshair => Cursor::Native(NSCursor::crosshairCursor),
            #[allow(deprecated)]
            MouseCursor::EResize => Cursor::Native(NSCursor::resizeRightCursor),
            #[allow(deprecated)]
            MouseCursor::NResize => Cursor::Native(NSCursor::resizeUpCursor),
            #[allow(deprecated)]
            MouseCursor::WResize => Cursor::Native(NSCursor::resizeLeftCursor),
            #[allow(deprecated)]
            MouseCursor::SResize => Cursor::Native(NSCursor::resizeDownCursor),
            #[allow(deprecated)]
            MouseCursor::EwResize | MouseCursor::ColResize => {
                Cursor::Native(NSCursor::resizeLeftRightCursor)
            }
            #[allow(deprecated)]
            MouseCursor::NsResize | MouseCursor::RowResize => {
                Cursor::Native(NSCursor::resizeUpDownCursor)
            }

            MouseCursor::Help => Cursor::Undocumented(sel!(_helpCursor)),
            MouseCursor::ZoomIn => Cursor::Undocumented(sel!(_zoomInCursor)),
            MouseCursor::ZoomOut => Cursor::Undocumented(sel!(_zoomOutCursor)),
            MouseCursor::NeResize => Cursor::Undocumented(sel!(_windowResizeNorthEastCursor)),
            MouseCursor::NwResize => Cursor::Undocumented(sel!(_windowResizeNorthWestCursor)),
            MouseCursor::SeResize => Cursor::Undocumented(sel!(_windowResizeSouthEastCursor)),
            MouseCursor::SwResize => Cursor::Undocumented(sel!(_windowResizeSouthWestCursor)),
            MouseCursor::NeswResize => {
                Cursor::Undocumented(sel!(_windowResizeNorthEastSouthWestCursor))
            }
            MouseCursor::NwseResize => {
                Cursor::Undocumented(sel!(_windowResizeNorthWestSouthEastCursor))
            }

            MouseCursor::Working | MouseCursor::PtrWorking => {
                Cursor::Undocumented(sel!(busyButClickableCursor))
            }

            _ => Cursor::Native(NSCursor::arrowCursor),
            // MouseCursor::Hidden => todo!(),
            // MouseCursor::Move => todo!(),
            // MouseCursor::AllScroll => todo!(),
            // MouseCursor::Cell => todo!(),
        }
    }
}

impl Cursor {
    pub fn load(&self) -> Retained<NSCursor> {
        match self {
            Cursor::Native(loader) => loader(),
            Cursor::Undocumented(sel) => {
                let class = NSCursor::class();

                // NOTE: class.responds_to does not yield the same result (probably because NSCursor overrides respondsToSelector)
                let responds_to: bool = unsafe { msg_send![class, respondsToSelector: *sel] };

                if !responds_to {
                    return NSCursor::arrowCursor();
                }

                let raw: *mut NSCursor = unsafe { class.send_message(*sel, ()) };
                let cursor = unsafe { Retained::retain(raw) };

                cursor.unwrap_or_else(NSCursor::arrowCursor)
            }
        }
    }
}
