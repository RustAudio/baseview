/// macOS window and event handling
///
/// Heavily inspired by implementation in https://github.com/antonok-edm/vst_window

use std::ffi::c_void;
use std::sync::Arc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyRegular, NSBackingStoreBuffered,
    NSRunningApplication, NSWindow, NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};

use objc::{msg_send, runtime::Object, sel, sel_impl};

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{
    Event, MouseEvent, WindowHandler, WindowOpenOptions, WindowScalePolicy,
    WindowInfo, Parent, Size, Point
};

use super::view::create_view;


/// Name of the field used to store the `WindowState` pointer in the `BaseviewNSView` class.
pub(super) const WINDOW_STATE_IVAR_NAME: &str = "WINDOW_STATE_IVAR_NAME";


pub struct Window {
    /// Only set if we created the parent window, i.e. we are running in
    /// parentless mode
    ns_window: Option<id>,
    /// Our subclassed NSView
    ns_view: id,
}


pub struct WindowHandle;


impl WindowHandle {
    pub fn app_run_blocking(self) {
        unsafe {
            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            app.run();
        }
    }
}

impl Window {
    pub fn open<H, B>(options: WindowOpenOptions, build: B) -> WindowHandle
        where H: WindowHandler,
              B: FnOnce(&mut Window) -> H,
              B: Send + 'static
    {
        let _pool = unsafe { NSAutoreleasePool::new(nil) };

        let mut window = match options.parent {
            Parent::WithParent(parent) => {
                if let RawWindowHandle::MacOS(handle) = parent {
                    let ns_view = handle.ns_view as *mut objc::runtime::Object;

                    unsafe {
                        let subview = create_view::<H>(&options);

                        let _: id = msg_send![ns_view, addSubview: subview];

                        Window {
                            ns_window: None,
                            ns_view: subview,
                        }
                    }
                } else {
                    panic!("Not a macOS window");
                }
            },
            Parent::AsIfParented => {
                let ns_view = unsafe {
                    create_view::<H>(&options)
                };

                Window {
                    ns_window: None,
                    ns_view,
                }
            },
            Parent::None => {
                let scaling = match options.scale {
                    WindowScalePolicy::SystemScaleFactor => get_scaling().unwrap_or(1.0),
                    WindowScalePolicy::ScaleFactor(scale) => scale
                };
        
                let window_info = WindowInfo::from_logical_size(options.size, scaling);

                let rect = NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(
                        window_info.logical_size().width as f64,
                        window_info.logical_size().height as f64
                    ),
                );

                unsafe {
                    let ns_window = NSWindow::alloc(nil)
                        .initWithContentRect_styleMask_backing_defer_(
                            rect,
                            NSWindowStyleMask::NSTitledWindowMask,
                            NSBackingStoreBuffered,
                            NO,
                        )
                        .autorelease();
                    ns_window.center();
                    ns_window.setTitle_(NSString::alloc(nil).init_str(&options.title));
                    ns_window.makeKeyAndOrderFront_(nil);

                    let subview = create_view::<H>(&options);

                    ns_window.setContentView_(subview);

                    let current_app = NSRunningApplication::currentApplication(nil);
                    current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

                    Window {
                        ns_window: Some(ns_window),
                        ns_view: subview,
                    }
                }
            },
        };

        let window_handler = build(&mut window);

        let window_state = WindowState {
            window,
            window_handler,
            size: options.size
        };

        let window_state = Arc::new(window_state);

        unsafe {
            (*window_state.window.ns_view).set_ivar(
                WINDOW_STATE_IVAR_NAME,
                Arc::into_raw(window_state.clone()) as *mut c_void
            );
        }

        WindowHandle
    }
}


pub(super) struct WindowState<H: WindowHandler> {
    pub(super) window: Window,
    pub(super) window_handler: H,
    size: Size,
}


impl <H: WindowHandler>WindowState<H> {
    /// Returns a mutable reference to an WindowState from an Objective-C callback.
    ///
    /// `clippy` has issues with this function signature, making the valid point that this could
    /// create multiple mutable references to the `WindowState`. However, in practice macOS
    /// blocks for the entire duration of each event callback, so this should be fine.
    #[allow(clippy::mut_from_ref)]
    pub(super) fn from_field(obj: &Object) -> &mut Self {
        unsafe {
            let state_ptr: *mut c_void = *obj.get_ivar(WINDOW_STATE_IVAR_NAME);
            &mut *(state_ptr as *mut Self)
        }
    }

    pub(super) fn trigger_cursor_moved(&mut self, location: NSPoint){
        let position = Point {
            x: (location.x / self.size.width),
            y: 1.0 - (location.y / self.size.height),
        };

        let event = Event::Mouse(MouseEvent::CursorMoved { position });

        self.window_handler.on_event(&mut self.window, event);
    }
}


unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let ns_window = self.ns_window.unwrap_or_else(
            ::std::ptr::null_mut
        ) as *mut c_void;

        RawWindowHandle::MacOS(MacOSHandle {
            ns_window,
            ns_view: self.ns_view as *mut c_void,
            ..MacOSHandle::empty()
        })
    }
}

fn get_scaling() -> Option<f64> {
    // TODO: find system scaling
    None
}
