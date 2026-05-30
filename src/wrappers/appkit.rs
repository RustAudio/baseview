mod timer;
mod view;
mod window;

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};
pub use timer::TimerHandle;
pub use view::*;
pub use window::*;

use raw_window_handle::RawWindowHandle;

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
