use std::ffi::c_void;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyRegular, NSBackingStoreBuffered, NSRunningApplication, NSView,
    NSWindow, NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};

use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Object, Sel},
    sel, sel_impl,
};

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{
    Event, KeyboardEvent, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowHandler,
    WindowOpenOptions, WindowScalePolicy, WindowInfo, Parent, Size, Point
};


/// Name of the field used to store the `EventDelegate` pointer in the `EventSubview` class.
const EVENT_DELEGATE_IVAR: &str = "EVENT_DELEGATE_IVAR";


pub struct Window {
    ns_window: id,
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
                match parent {
                    RawWindowHandle::MacOS(handle) => {
                        let ns_window = handle.ns_window as *mut objc::runtime::Object;
                        let ns_view = handle.ns_view as *mut objc::runtime::Object;

                        Window {
                            ns_window,
                            ns_view,
                        }
                    },
                    _ => {
                        panic!("Not a macOS window");
                    }
                }
            },
            Parent::AsIfParented => {
                unimplemented!()
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

                    let ns_view = NSView::alloc(nil).init();
                    ns_window.setContentView_(ns_view);

                    let current_app = NSRunningApplication::currentApplication(nil);
                    current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

                    Window {
                        ns_window,
                        ns_view,
                    }
                }
            },
        };

        let handler = build(&mut window);

        Self::setup_event_delegate(options, window, handler);

        WindowHandle
    }

    fn setup_event_delegate<H: WindowHandler>(
        window_options: WindowOpenOptions,
        window: Window,
        window_handler: H
    ){
        unsafe {
            let mut class = ClassDecl::new("EventSubview", class!(NSView)).unwrap();

            class.add_method(sel!(dealloc), dealloc::<H> as extern "C" fn(&Object, Sel));

            class.add_method(
                sel!(mouseDown:),
                mouse_down::<H> as extern "C" fn(&Object, Sel, id),
            );

            class.add_ivar::<*mut c_void>(EVENT_DELEGATE_IVAR);

            let class = class.register();
            let event_subview: id = msg_send![class, alloc];

            let size = window_options.size;

            event_subview.initWithFrame_(NSRect::new(
                NSPoint::new(0., 0.),
                NSSize::new(size.width, size.height),
            ));
            let _: id = msg_send![window.ns_view, addSubview: event_subview];

            let event_delegate = EventDelegate {
                window,
                window_handler,
                size
            };

            let event_delegate_reference = Box::into_raw(Box::new(event_delegate));

            (*event_subview).set_ivar(EVENT_DELEGATE_IVAR, event_delegate_reference as *mut c_void);
        }
    }
}


struct EventDelegate<H: WindowHandler> {
    window: Window,
    window_handler: H,
    size: Size,
}


impl <H: WindowHandler>EventDelegate<H> {
    /// Returns a mutable reference to an EventDelegate from an Objective-C callback.
    ///
    /// `clippy` has issues with this function signature, making the valid point that this could
    /// create multiple mutable references to the `EventDelegate`. However, in practice macOS
    /// blocks for the entire duration of each event callback, so this should be fine.
    #[allow(clippy::mut_from_ref)]
    fn from_field(obj: &Object) -> &mut Self {
        unsafe {
            let delegate_ptr: *mut c_void = *obj.get_ivar(EVENT_DELEGATE_IVAR);
            &mut *(delegate_ptr as *mut Self)
        }
    }
}


extern "C" fn dealloc<H: WindowHandler>(this: &Object, _sel: Sel) {
    unsafe {
        let delegate_ptr: *mut c_void = *this.get_ivar(EVENT_DELEGATE_IVAR);
        Box::from_raw(delegate_ptr as *mut EventDelegate<H>);
    }
}


extern "C" fn mouse_down<H: WindowHandler>(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate: &mut EventDelegate<H> = EventDelegate::from_field(this);

    let position = Point {
        x: (location.x / delegate.size.width),
        y: 1.0 - (location.y / delegate.size.height),
    };
    let event = Event::Mouse(MouseEvent::CursorMoved { position });
    delegate.window_handler.on_event(&mut delegate.window, event);

    let event = Event::Mouse(MouseEvent::ButtonPressed(MouseButton::Left));
    delegate.window_handler.on_event(&mut delegate.window, event);
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

fn get_scaling() -> Option<f64> {
    // TODO: find system scaling
    None
}
