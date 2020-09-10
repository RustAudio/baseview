use std::ffi::c_void;
use std::sync::mpsc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyRegular, NSBackingStoreBuffered, NSRunningApplication, NSView,
    NSWindow, NSWindowStyleMask,
};
use cocoa::base::{nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{
    AppWindow, Event, MouseButton, MouseScroll, RawWindow, WindowInfo, WindowOpenOptions,
    MouseEvent, KeyboardEvent, WindowEvent, FileDropEvent, Keycode, ScrollDelta,
};

pub struct Window<A: AppWindow> {
    app_window: A,
    app_message_rx: mpsc::Receiver<A::AppMessage>,
}

impl<A: AppWindow> Window<A> {
    pub fn open(options: WindowOpenOptions, app_message_rx: mpsc::Receiver<A::AppMessage>) -> Self {
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

            let raw_window = RawWindow {
                raw_window_handle: RawWindowHandle::MacOS(MacOSHandle {
                    ns_window: window as *mut c_void,
                    ns_view: app as *mut c_void,
                    ..raw_window_handle::macos::MacOSHandle::empty()
                }),
            };

            let window_info = WindowInfo {
                width: options.width as u32,
                height: options.height as u32,
                scale_factor: 1.0,
            };

            let app_window = A::build(raw_window, &window_info);

            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            app.run();

            Window {
                app_window,
                app_message_rx,
            }
        }
    }
}
