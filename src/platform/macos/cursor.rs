use crate::MouseCursor;
use objc2::__framework_prelude::Retained;
use objc2::runtime::{MessageReceiver, Sel};
use objc2::{msg_send, sel, AnyThread, ClassType, Message};
use objc2_app_kit::{NSCursor, NSImage};
use objc2_foundation::{NSPoint, NSSize};
use std::cell::{Cell, LazyCell, RefCell};

pub struct CursorManager {
    is_inside: Cell<bool>,
    current: Cell<MouseCursor>,
    current_cursor: RefCell<Retained<NSCursor>>,
    empty: LazyCell<Retained<NSCursor>>,
}

impl CursorManager {
    pub fn new() -> Self {
        Self {
            current: MouseCursor::Default.into(),
            current_cursor: NSCursor::arrowCursor().into(),
            empty: LazyCell::new(Self::create_empty_cursor),
            is_inside: Cell::new(false),
        }
    }

    fn create_empty_cursor() -> Retained<NSCursor> {
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(0.0, 0.0));
        NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, NSPoint::ZERO)
    }

    pub fn set_is_inside(&self, is_inside: bool) {
        self.is_inside.set(is_inside);
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        if self.current.get() == cursor {
            self.update_to_current_cursor();
            return;
        }

        self.current_cursor.replace(self.load(cursor.into()));
        self.current.set(cursor);

        if self.is_inside.get() {
            self.update_to_current_cursor();
        }
    }

    pub fn update_to_current_cursor(&self) {
        //eprintln!("cursor set!");
        NSCursor::crosshairCursor().set();
        //self.current_cursor.borrow().set();
    }

    fn load(&self, cursor: Cursor) -> Retained<NSCursor> {
        match cursor {
            Cursor::Native(loader) => loader(),
            Cursor::Undocumented(sel) => {
                let class = NSCursor::class();

                // NOTE: class.responds_to does not yield the same result (probably because NSCursor overrides respondsToSelector)
                let responds_to: bool = unsafe { msg_send![class, respondsToSelector: sel] };

                if !responds_to {
                    return NSCursor::arrowCursor();
                }

                let raw: *mut NSCursor = unsafe { class.send_message(sel, ()) };
                let cursor = unsafe { Retained::retain(raw) };

                cursor.unwrap_or_else(NSCursor::arrowCursor)
            }
            Cursor::Hidden => self.empty.retain(),
        }
    }
}

#[derive(Debug)]
enum Cursor {
    Native(fn() -> Retained<NSCursor>),
    Undocumented(Sel),
    Hidden,
}

impl From<MouseCursor> for Cursor {
    #[expect(deprecated, reason = "TODO: resize curosrs are deprecated")]
    fn from(cursor: MouseCursor) -> Self {
        match cursor {
            MouseCursor::Default => Cursor::Native(NSCursor::arrowCursor),
            MouseCursor::Hand => Cursor::Native(NSCursor::pointingHandCursor),
            MouseCursor::HandGrabbing => Cursor::Native(NSCursor::closedHandCursor),
            MouseCursor::Text => Cursor::Native(NSCursor::IBeamCursor),
            MouseCursor::VerticalText => Cursor::Native(NSCursor::IBeamCursorForVerticalLayout),
            MouseCursor::Copy => Cursor::Native(NSCursor::dragCopyCursor),
            MouseCursor::Alias => Cursor::Native(NSCursor::dragLinkCursor),
            MouseCursor::NotAllowed | MouseCursor::PtrNotAllowed => {
                Cursor::Native(NSCursor::operationNotAllowedCursor)
            }
            MouseCursor::Crosshair => Cursor::Native(NSCursor::crosshairCursor),
            MouseCursor::EResize => Cursor::Native(NSCursor::resizeRightCursor),
            MouseCursor::NResize => Cursor::Native(NSCursor::resizeUpCursor),
            MouseCursor::WResize => Cursor::Native(NSCursor::resizeLeftCursor),
            MouseCursor::SResize => Cursor::Native(NSCursor::resizeDownCursor),
            MouseCursor::EwResize | MouseCursor::ColResize => {
                Cursor::Native(NSCursor::resizeLeftRightCursor)
            }
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

            MouseCursor::Move => Cursor::Native(NSCursor::arrowCursor),
            MouseCursor::AllScroll => Cursor::Native(NSCursor::arrowCursor),
            MouseCursor::Cell => Cursor::Native(NSCursor::crosshairCursor),
            MouseCursor::Hidden => Cursor::Hidden,
        }
    }
}
