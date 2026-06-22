use crate::wrappers::appkit::{View, ViewImpl};
use dpi::LogicalSize;
use objc2::rc::Retained;
use objc2::{msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize};

pub fn create_window(size: LogicalSize<f64>, mtm: MainThreadMarker) -> Retained<NSWindow> {
    let rect = NSRect::new(NSPoint::ZERO, NSSize { width: size.width, height: size.height });

    // SAFETY: This is safe because of the setReleasedWhenClosed(false) below
    let ns_window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect,
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    // SAFETY: setReleasedWhenClosed is always safe to call with `false` (worst case is a memory leak)
    unsafe { ns_window.setReleasedWhenClosed(false) };

    ns_window
}

pub fn set_delegate(window: &NSWindow, delegate: &View<impl ViewImpl>) {
    let () = unsafe { msg_send![window, setDelegate: delegate] };
}
