/// macOS window handling
///
/// Inspired by implementation in https://github.com/antonok-edm/vst_window

use std::ffi::c_void;
use std::sync::Arc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyRegular,
    NSBackingStoreBuffered, NSWindow, NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};

use objc::{msg_send, runtime::Object, sel, sel_impl};

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{
    Event, Parent, WindowHandler, WindowOpenOptions, WindowScalePolicy,
    WindowInfo
};

use super::view::create_view;


/// Name of the field used to store the `WindowState` pointer in the custom
/// view class.
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
            // Get reference to already created shared NSApplication object
            // and run the main loop
            NSApp().run();
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
                // It seems prudent to run NSApp() here before doing other
                // work. It runs [NSApplication sharedApplication], which is
                // what is run at the very start of the Xcode-generated main
                // function of a cocoa app according to:
                // https://developer.apple.com/documentation/appkit/nsapplication
                unsafe {
                    let app = NSApp();
                    app.setActivationPolicy_(
                        NSApplicationActivationPolicyRegular
                    );
                }

                let scaling = match options.scale {
                    WindowScalePolicy::ScaleFactor(scale) => scale,
                    WindowScalePolicy::SystemScaleFactor => {
                        get_scaling().unwrap_or(1.0)
                    },
                };
        
                let window_info = WindowInfo::from_logical_size(
                    options.size,
                    scaling
                );

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

                    let title = NSString::alloc(nil)
                        .init_str(&options.title)
                        .autorelease();
                    ns_window.setTitle_(title);

                    ns_window.makeKeyAndOrderFront_(nil);

                    let subview = create_view::<H>(&options);

                    ns_window.setContentView_(subview);

                    Window {
                        ns_window: Some(ns_window),
                        ns_view: subview,
                    }
                }
            },
        };

        let window_handler = build(&mut window);

        let window_state_arc = Arc::new(WindowState {
            window,
            window_handler,
        });

        let window_state_pointer = Arc::into_raw(
            window_state_arc.clone()
        ) as *mut c_void;

        unsafe {
            (*window_state_arc.window.ns_view).set_ivar(
                WINDOW_STATE_IVAR_NAME,
                window_state_pointer
            );
        }

        WindowHandle
    }
}


pub(super) struct WindowState<H: WindowHandler> {
    window: Window,
    window_handler: H,
}


impl <H: WindowHandler>WindowState<H> {
    /// Returns a mutable reference to a WindowState from an Objective-C field
    ///
    /// Don't use this to create two simulataneous references to a single
    /// WindowState. Apparently, macOS blocks for the duration of an event,
    /// callback, meaning that this shouldn't be a problem in practice.
    pub(super) unsafe fn from_field(obj: &Object) -> &mut Self {
        let state_ptr: *mut c_void = *obj.get_ivar(WINDOW_STATE_IVAR_NAME);

        &mut *(state_ptr as *mut Self)
    }

    pub(super) fn trigger_event(&mut self, event: Event){
        self.window_handler.on_event(&mut self.window, event);
    }
}


unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let ns_window = self.ns_window.unwrap_or(
            ::std::ptr::null_mut()
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
