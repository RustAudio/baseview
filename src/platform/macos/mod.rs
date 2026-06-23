mod context;
mod cursor;
mod keyboard;
mod view;
mod window;

use crate::wrappers::appkit::RwhPinkyPromiseNSView;
pub use context::WindowContext;
use objc2::rc::Weak;
use raw_window_handle::{AppKitWindowHandle, DisplayHandle};
use std::fmt;
use std::fmt::Formatter;
use std::ptr::NonNull;
pub use window::*;

#[cfg(feature = "opengl")]
pub mod gl;

#[derive(Clone)]
pub struct PlatformHandle {
    view: Weak<RwhPinkyPromiseNSView>,
}

impl PlatformHandle {
    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let view = self.view.load()?;
        let ns_view = NonNull::from(&view as &RwhPinkyPromiseNSView).cast();
        let handle = AppKitWindowHandle::new(ns_view);

        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::appkit()
    }
}

impl fmt::Debug for PlatformHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        struct PtrFmt<T>(T);
        impl<T: fmt::Pointer> fmt::Debug for PtrFmt<T> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                fmt::Pointer::fmt(&self.0, f)
            }
        }

        f.debug_struct("PlatformHandle (AppKit)").field("ns_view", &PtrFmt(&self.view)).finish()
    }
}
