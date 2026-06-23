mod context;
mod cursor;
mod keyboard;
mod view;
mod window;

use crate::platform::macos::view::BaseviewView;
use crate::wrappers::appkit::View;
pub use context::WindowContext;
use dispatch2::MainThreadBound;
use objc2::rc::Weak;
use objc2::MainThreadMarker;
use raw_window_handle::DisplayHandle;
use std::fmt;
use std::fmt::Formatter;
pub use window::*;

#[cfg(feature = "opengl")]
pub mod gl;

pub struct PlatformHandle {
    inner: MainThreadBound<Weak<View<BaseviewView>>>,
}

impl PlatformHandle {
    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let mtm = MainThreadMarker::new()?;
        let view = self.inner.get(mtm);

        View::window_handle_from_weak(view)
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::appkit()
    }
}

impl Clone for PlatformHandle {
    fn clone(&self) -> Self {
        // upstream this in objc2/dispatch2 someday
        // SAFETY: The use of this marker is only to access the thread-safe Retained impl
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let view = self.inner.get(mtm);
        Self { inner: MainThreadBound::new(view.clone(), mtm) }
    }
}

impl fmt::Debug for PlatformHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        struct PtrFmt<'a>(&'a PlatformHandle);
        impl fmt::Debug for PtrFmt<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                // upstream this in objc2/dispatch2 someday
                // SAFETY: The use of this marker is only to access the thread-safe Retained impl
                let mtm = unsafe { MainThreadMarker::new_unchecked() };
                match self.0.inner.get(mtm).load() {
                    Some(retained) => fmt::Pointer::fmt(&retained, f),
                    _ => f.write_str("(gone)"),
                }
            }
        }

        f.debug_struct("PlatformHandle (AppKit)").field("ns_view", &PtrFmt(&self)).finish()
    }
}
