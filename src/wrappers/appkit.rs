mod timer;
mod view;
mod window;

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};
use objc2_core_foundation::CFUUID;
use std::ffi::CString;
pub use timer::TimerHandle;
pub use view::*;
pub use window::*;

use raw_window_handle::RawWindowHandle;

fn new_class_name(prefix: &str) -> CString {
    // PANIC: CFUUIDCreate is not documented to return NULL.
    let uuid = CFUUID::new(None).unwrap();
    // PANIC: CFUUIDCreateString is not documented to return NULL.
    let uuid_str = CFUUID::new_string(None, Some(&uuid)).unwrap();

    let class_name = format!("{prefix}{uuid_str}");
    // PANIC: This cannot have any NULL bytes
    CString::new(class_name).unwrap()
}

pub fn extract_raw_window_handle(
    handle: RawWindowHandle,
) -> (Option<Retained<NSWindow>>, Option<Retained<NSView>>) {
    let RawWindowHandle::AppKit(handle) = handle else {
        panic!("Not a macOS window");
    };

    let parent_window = unsafe { Retained::retain(handle.ns_window as *mut NSWindow) };
    let parent_view = unsafe { Retained::retain(handle.ns_view as *mut NSView) };

    (parent_window, parent_view)
}
