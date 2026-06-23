use objc2::__framework_prelude::Retained;
use objc2::rc::Weak;
use objc2::{Encoding, Message, RefEncode};
use objc2_app_kit::NSView;
use raw_window_handle::AppKitWindowHandle;
use std::ptr::NonNull;

#[repr(C)]
pub struct RwhPinkyPromiseNSView {
    must_protecc: NSView,
}

impl RwhPinkyPromiseNSView {
    pub fn new(view: Retained<NSView>) -> Retained<RwhPinkyPromiseNSView> {
        // SAFETY: Safe due to #[repr(C)] and just wrapping an NSView
        unsafe { Retained::cast_unchecked(view) }
    }

    pub fn window_handle(&self) -> raw_window_handle::WindowHandle {
        let handle = AppKitWindowHandle::new(NonNull::from(&self.must_protecc).cast());

        // SAFETY: This is safe, as this type guarantees we're an actual NSView and is borrowed by &self
        unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) }
    }
}

// SAFETY: Safe due to #[repr(C)] and just wrapping an NSView
unsafe impl RefEncode for RwhPinkyPromiseNSView {
    const ENCODING_REF: Encoding = NSView::ENCODING_REF;
}

// SAFETY: same as above
unsafe impl Message for RwhPinkyPromiseNSView {}

// SAFETY: RWH's Pinky promise!
unsafe impl Send for RwhPinkyPromiseNSView {}
// SAFETY: RWH's Pinky promise!
unsafe impl Sync for RwhPinkyPromiseNSView {}
