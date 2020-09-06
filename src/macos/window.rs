use std::ffi::c_void;
use std::sync::mpsc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyRegular, NSBackingStoreBuffered, NSRunningApplication, NSView,
    NSWindow, NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};

use objc::declare::ClassDecl;

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{AppWindow, Event, MouseButtonID, MouseScroll, WindowOpenOptions};

pub struct Window {
    app: id,
    window: id,
    view: id,
}

impl Window {
    pub fn open(options: WindowOpenOptions) -> Self {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);

            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            let rect = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(options.width as f64, options.height as f64),
            );

            let window = NSWindow::alloc(nil)
                .initWithContentRect_styleMask_backing_defer_(
                    rect,
                    NSWindowStyleMask::NSTitledWindowMask,
                    NSBackingStoreBuffered,
                    NO,
                )
                .autorelease();
            window.center();
            window.setTitle_(NSString::alloc(nil).init_str(options.title));
            window.makeKeyAndOrderFront_(nil);

            let view = NSView::alloc(nil).init();
            window.setContentView_(view);

            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

            Window { app, window, view }
        }
    }

    pub fn run<A: AppWindow>(self, app_window: A, app_message_rx: mpsc::Receiver<A::AppMessage>) {
        unsafe {
            self.app.run();
        }
    }
}

unsafe impl raw_window_handle::HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        RawWindowHandle::MacOS(MacOSHandle {
            ns_window: self.window as *mut c_void,
            ns_view: self.app as *mut c_void,
            ..raw_window_handle::macos::MacOSHandle::empty()
        })
    }
}
