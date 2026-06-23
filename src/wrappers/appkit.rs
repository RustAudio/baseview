mod notification_center;
mod rwh_view;
mod timer;
mod view;
mod window;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_foundation::CFUUID;
use std::ffi::CString;

pub use notification_center::*;
pub use rwh_view::*;
pub use timer::TimerHandle;
pub use view::*;
pub use window::*;

use raw_window_handle::{RawWindowHandle, WindowHandle};

fn new_class_name(prefix: &str) -> CString {
    // PANIC: CFUUIDCreate is not documented to return NULL.
    let uuid = CFUUID::new(None).unwrap();
    // PANIC: CFUUIDCreateString is not documented to return NULL.
    let uuid_str = CFUUID::new_string(None, Some(&uuid)).unwrap();

    let class_name = format!("{prefix}{uuid_str}");
    // PANIC: This cannot have any NULL bytes
    CString::new(class_name).unwrap()
}

pub fn extract_raw_window_handle(handle: WindowHandle) -> Option<Retained<NSView>> {
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        panic!("Not a macOS window");
    };

    unsafe { Retained::retain(handle.ns_view.as_ptr() as *mut NSView) }
}
