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
    AppWindow, Event, FileDropEvent, KeyCode, KeyboardEvent, MouseButton, MouseEvent, ScrollDelta,
    WindowEvent, WindowOpenOptions, WindowState,
};

pub struct Window<A: AppWindow> {
    app_window: A,
    app_message_rx: mpsc::Receiver<A::AppMessage>,
    window_state: WindowState,
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

            let raw_handle = RawWindowHandle::MacOS(MacOSHandle {
                ns_window: window as *mut c_void,
                ns_view: app as *mut c_void,
                ..raw_window_handle::macos::MacOSHandle::empty()
            });

            let mut window_state = WindowState::new(
                options.width as u32,
                options.height as u32,
                1.0, // scaling
                raw_handle,
            );

            let app_window = A::build(&mut window_state);

            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            app.run();

            Window {
                app_window,
                app_message_rx,
                window_state,
            }
        }
    }
}
