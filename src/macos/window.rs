use std::ffi::c_void;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyRegular, NSBackingStoreBuffered, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext, __CFRunLoopTimer, kCFRunLoopDefaultMode,
};
use keyboard_types::KeyboardEvent;

use objc::{msg_send, runtime::Object, sel, sel_impl};

use raw_window_handle::{macos::MacOSHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{
    Event, EventStatus, WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions,
    WindowScalePolicy,
};

use super::keyboard::KeyboardState;
use super::view::{create_view, BASEVIEW_STATE_IVAR};

pub struct WindowHandle {
    raw_window_handle: Option<RawWindowHandle>,
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,

    // Ensure handle is !Send
    _phantom: PhantomData<*mut ()>,
}

impl WindowHandle {
    pub fn close(&mut self) {
        if let Some(_) = self.raw_window_handle.take() {
            self.close_requested.store(true, Ordering::Relaxed);
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed)
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        if let Some(raw_window_handle) = self.raw_window_handle {
            if self.is_open.load(Ordering::Relaxed) {
                return raw_window_handle;
            }
        }

        RawWindowHandle::MacOS(MacOSHandle { ..MacOSHandle::empty() })
    }
}

struct ParentHandle {
    _close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl ParentHandle {
    pub fn new(raw_window_handle: RawWindowHandle) -> (Self, WindowHandle) {
        let close_requested = Arc::new(AtomicBool::new(false));
        let is_open = Arc::new(AtomicBool::new(true));

        let handle = WindowHandle {
            raw_window_handle: Some(raw_window_handle),
            close_requested: Arc::clone(&close_requested),
            is_open: Arc::clone(&is_open),
            _phantom: PhantomData::default(),
        };

        (Self { _close_requested: close_requested, is_open }, handle)
    }

    /*
    pub fn parent_did_drop(&self) -> bool {
        self.close_requested.load(Ordering::Relaxed)
    }
    */
}

impl Drop for ParentHandle {
    fn drop(&mut self) {
        self.is_open.store(false, Ordering::Relaxed);
    }
}

pub struct Window {
    /// Only set if we created the parent window, i.e. we are running in
    /// parentless mode
    ns_app: Option<id>,
    /// Only set if we created the parent window, i.e. we are running in
    /// parentless mode
    ns_window: Option<id>,
    /// Our subclassed NSView
    ns_view: id,
    close_requested: bool,
}

impl Window {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let pool = unsafe { NSAutoreleasePool::new(nil) };

        let handle = if let RawWindowHandle::MacOS(handle) = parent.raw_window_handle() {
            handle
        } else {
            panic!("Not a macOS window");
        };

        let ns_view = unsafe { create_view(&options) };

        let window = Window { ns_app: None, ns_window: None, ns_view, close_requested: false };

        let window_handle = Self::init(true, window, build);

        unsafe {
            let _: id = msg_send![handle.ns_view as *mut Object, addSubview: ns_view];
            let () = msg_send![ns_view as id, release];

            let () = msg_send![pool, drain];
        }

        window_handle
    }

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let pool = unsafe { NSAutoreleasePool::new(nil) };

        let ns_view = unsafe { create_view(&options) };

        let window = Window { ns_app: None, ns_window: None, ns_view, close_requested: false };

        let window_handle = Self::init(true, window, build);

        unsafe {
            let () = msg_send![pool, drain];
        }

        window_handle
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let pool = unsafe { NSAutoreleasePool::new(nil) };

        // It seems prudent to run NSApp() here before doing other
        // work. It runs [NSApplication sharedApplication], which is
        // what is run at the very start of the Xcode-generated main
        // function of a cocoa app according to:
        // https://developer.apple.com/documentation/appkit/nsapplication
        let app = unsafe { NSApp() };

        unsafe {
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
        }

        let scaling = match options.scale {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        };

        let window_info = WindowInfo::from_logical_size(options.size, scaling);

        let rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(
                window_info.logical_size().width as f64,
                window_info.logical_size().height as f64,
            ),
        );

        let ns_window = unsafe {
            let ns_window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                rect,
                NSWindowStyleMask::NSTitledWindowMask
                    | NSWindowStyleMask::NSClosableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask,
                NSBackingStoreBuffered,
                NO,
            );
            ns_window.center();

            let title = NSString::alloc(nil).init_str(&options.title).autorelease();
            ns_window.setTitle_(title);

            ns_window.makeKeyAndOrderFront_(nil);

            ns_window
        };

        let ns_view = unsafe { create_view(&options) };

        let window = Window {
            ns_app: Some(app),
            ns_window: Some(ns_window),
            ns_view,
            close_requested: false,
        };

        let _ = Self::init(false, window, build);

        unsafe {
            ns_window.setContentView_(ns_view);

            let () = msg_send![ns_view as id, release];
            let () = msg_send![pool, drain];

            app.run();
        }
    }

    fn init<H, B>(parented: bool, mut window: Window, build: B) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let window_handler = Box::new(build(&mut crate::Window::new(&mut window)));

        let (parent_handle, window_handle) = ParentHandle::new(window.raw_window_handle());
        let parent_handle = if parented { Some(parent_handle) } else { None };

        let retain_count_after_build: usize = unsafe { msg_send![window.ns_view, retainCount] };

        let window_state_ptr = Box::into_raw(Box::new(WindowState {
            window,
            window_handler,
            keyboard_state: KeyboardState::new(),
            frame_timer: None,
            retain_count_after_build,
            _parent_handle: parent_handle,
        }));

        unsafe {
            (*(*window_state_ptr).window.ns_view)
                .set_ivar(BASEVIEW_STATE_IVAR, window_state_ptr as *mut c_void);

            WindowState::setup_timer(window_state_ptr);
        }

        window_handle
    }

    pub fn close(&mut self) {
        self.close_requested = true;
    }
}

pub(super) struct WindowState {
    window: Window,
    window_handler: Box<dyn WindowHandler>,
    keyboard_state: KeyboardState,
    frame_timer: Option<CFRunLoopTimer>,
    _parent_handle: Option<ParentHandle>,
    pub retain_count_after_build: usize,
}

impl WindowState {
    /// Returns a mutable reference to a WindowState from an Objective-C field
    ///
    /// Don't use this to create two simulataneous references to a single
    /// WindowState. Apparently, macOS blocks for the duration of an event,
    /// callback, meaning that this shouldn't be a problem in practice.
    pub(super) unsafe fn from_field(obj: &Object) -> &mut Self {
        let state_ptr: *mut c_void = *obj.get_ivar(BASEVIEW_STATE_IVAR);

        &mut *(state_ptr as *mut Self)
    }

    pub(super) fn trigger_event(&mut self, event: Event) -> EventStatus {
        self.window_handler.on_event(&mut crate::Window::new(&mut self.window), event)
    }

    pub(super) fn trigger_frame(&mut self) {
        self.window_handler.on_frame(&mut crate::Window::new(&mut self.window));

        let mut do_close = false;

        /* FIXME: Is it even necessary to check if the parent dropped the handle
        // in MacOS?
        // Check if the parent handle was dropped
        if let Some(parent_handle) = &self.parent_handle {
            if parent_handle.parent_did_drop() {
                do_close = true;
                self.window.close_requested = false;
            }
        }
        */

        // Check if the user requested the window to close
        if self.window.close_requested {
            do_close = true;
            self.window.close_requested = false;
        }

        if do_close {
            unsafe {
                if let Some(ns_window) = self.window.ns_window.take() {
                    ns_window.close();
                } else {
                    // FIXME: How do we close a non-parented window? Is this even
                    // possible in a DAW host usecase?
                }
            }
        }
    }

    pub(super) fn process_native_key_event(&mut self, event: *mut Object) -> Option<KeyboardEvent> {
        self.keyboard_state.process_native_event(event)
    }

    /// Don't call until WindowState pointer is stored in view
    unsafe fn setup_timer(window_state_ptr: *mut WindowState) {
        extern "C" fn timer_callback(_: *mut __CFRunLoopTimer, window_state_ptr: *mut c_void) {
            unsafe {
                let window_state = &mut *(window_state_ptr as *mut WindowState);

                window_state.trigger_frame();
            }
        }

        let mut timer_context = CFRunLoopTimerContext {
            version: 0,
            info: window_state_ptr as *mut c_void,
            retain: None,
            release: None,
            copyDescription: None,
        };

        let timer = CFRunLoopTimer::new(0.0, 0.015, 0, 0, timer_callback, &mut timer_context);

        CFRunLoop::get_current().add_timer(&timer, kCFRunLoopDefaultMode);

        let window_state = &mut *(window_state_ptr);

        window_state.frame_timer = Some(timer);
    }

    /// Call when freeing view
    pub(super) unsafe fn stop(&mut self) {
        if let Some(frame_timer) = self.frame_timer.take() {
            CFRunLoop::get_current().remove_timer(&frame_timer, kCFRunLoopDefaultMode);
        }

        self.trigger_event(Event::Window(WindowEvent::WillClose));

        // If in non-parented mode, we want to also quit the app altogether
        if let Some(app) = self.window.ns_app.take() {
            app.stop_(app);
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let ns_window = self.ns_window.unwrap_or(::std::ptr::null_mut()) as *mut c_void;

        RawWindowHandle::MacOS(MacOSHandle {
            ns_window,
            ns_view: self.ns_view as *mut c_void,
            ..MacOSHandle::empty()
        })
    }
}
