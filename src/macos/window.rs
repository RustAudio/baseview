use std::ffi::c_void;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyRegular, NSBackingStoreBuffered, NSRunningApplication, NSView,
    NSWindow, NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{
    Event, FileDropEvent, KeyboardEvent, MouseButton, MouseEvent, ScrollDelta, WindowEvent,
    WindowHandler, WindowOpenOptions,
};

pub struct Window {
    ns_window: id,
    ns_view: id,
}

impl Window {
    pub fn open<H: WindowHandler>(options: WindowOpenOptions) -> WindowHandle {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);

            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            let rect = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(options.width as f64, options.height as f64),
            );

            let ns_window = NSWindow::alloc(nil)
                .initWithContentRect_styleMask_backing_defer_(
                    rect,
                    NSWindowStyleMask::NSTitledWindowMask,
                    NSBackingStoreBuffered,
                    NO,
                )
                .autorelease();
            ns_window.center();
            ns_window.setTitle_(NSString::alloc(nil).init_str(options.title));
            ns_window.makeKeyAndOrderFront_(nil);

            let ns_view = NSView::alloc(nil).init();
            ns_window.setContentView_(ns_view);

            let mut window = Window { ns_window, ns_view };

            let handler = H::build(&mut window);

            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            app.run();

            WindowHandle
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::MacOS(MacOSHandle {
            ns_window: self.ns_window as *mut c_void,
            ns_view: self.ns_view as *mut c_void,
            ..MacOSHandle::empty()
        })
    }
}

pub struct WindowHandle;
