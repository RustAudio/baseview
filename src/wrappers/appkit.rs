mod notification_center;
mod timer;
mod view;
mod window;

pub use notification_center::*;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_foundation::CFUUID;
use std::ffi::CString;
use std::fmt::{Display, Formatter};
pub use timer::TimerHandle;
pub use view::*;
pub use window::*;

use raw_window_handle::{HandleError, RawWindowHandle, WindowHandle};

fn new_class_name(prefix: &str) -> CString {
    // PANIC: CFUUIDCreate is not documented to return NULL.
    let Some(uuid) = CFUUID::new(None) else { unreachable!() };
    // PANIC: CFUUIDCreateString is not documented to return NULL.
    let Some(uuid_str) = CFUUID::new_string(None, Some(&uuid)) else { unreachable!() };

    let class_name = format!("{prefix}{uuid_str}");
    // PANIC: This cannot have any NULL bytes
    let Ok(class_name) = CString::new(class_name) else { unreachable!() };
    class_name
}

pub fn extract_raw_window_handle(
    raw_handle: WindowHandle,
) -> Result<Retained<NSView>, ParentWindowHandleError> {
    let raw_handle = raw_handle.as_raw();
    let RawWindowHandle::AppKit(handle) = raw_handle else {
        return Err(ParentWindowHandleError::UnsupportedWindowHandleType(raw_handle));
    };

    (unsafe { Retained::retain(handle.ns_view.as_ptr() as *mut NSView) })
        .ok_or(ParentWindowHandleError::NullViewPtr)
}

pub enum ParentWindowHandleError {
    HandleError(HandleError),
    UnsupportedWindowHandleType(RawWindowHandle),
    NullViewPtr,
}

impl From<HandleError> for ParentWindowHandleError {
    fn from(value: HandleError) -> Self {
        Self::HandleError(value)
    }
}

impl Display for ParentWindowHandleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParentWindowHandleError::HandleError(e) => e.fmt(f),
            ParentWindowHandleError::UnsupportedWindowHandleType(h) => {
                write!(f, "Unsupported window handle type on macOS (AppKit): {h:?}")
            }
            ParentWindowHandleError::NullViewPtr => f.write_str("NSView pointer is NULL"),
        }
    }
}
