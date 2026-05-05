use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use keyboard_types::KeyboardEvent;
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEvent, NSPasteboard,
    NSPasteboardTypeString, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::{
    kCFAllocatorDefault, kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopTimer, CFRunLoopTimerContext,
};
use objc2_foundation::{NSNotificationCenter, NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use crate::{
    Event, EventStatus, MouseCursor, Size, WindowHandler, WindowInfo, WindowOpenOptions,
    WindowScalePolicy,
};

use super::keyboard::KeyboardState;
use super::view::{create_view, BASEVIEW_STATE_IVAR};

#[cfg(feature = "opengl")]
use crate::gl::{GlConfig, GlContext};
use crate::macos::RetainedCell;

pub struct WindowHandle {
    state: Rc<WindowState>,
}

impl WindowHandle {
    pub fn close(&mut self) {
        self.state.window_inner.close();
    }

    pub fn is_open(&self) -> bool {
        self.state.window_inner.open.get()
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.state.window_inner.raw_window_handle()
    }
}

pub(super) struct WindowInner {
    open: Cell<bool>,

    /// Only set if we created the parent window, i.e. we are running in
    /// parentless mode
    ns_app: RetainedCell<NSApplication>,
    /// Only set if we created the parent window, i.e. we are running in
    /// parentless mode
    ns_window: RetainedCell<NSWindow>,

    /// Only set when running in parented mode.
    parent_ns_window: RetainedCell<NSWindow>,

    /// Our subclassed NSView
    ns_view: RetainedCell<NSView>,

    #[cfg(feature = "opengl")]
    pub(super) gl_context: Option<GlContext>,
}

impl WindowInner {
    pub(super) fn close(&self) {
        if self.open.get() {
            self.open.set(false);
            let Some(ns_view) = self.ns_view.take() else {
                return;
            };

            unsafe {
                // Take back ownership of the NSView's Rc<WindowState>
                let state_ptr: *const c_void = *ns_view
                    .class()
                    .instance_variable(BASEVIEW_STATE_IVAR)
                    .unwrap()
                    .load::<*const c_void>(&ns_view);

                let window_state = Rc::from_raw(state_ptr as *mut WindowState);

                // Cancel the frame timer
                if let Some(frame_timer) = window_state.frame_timer.take() {
                    if let Some(run_loop) = CFRunLoop::current() {
                        run_loop.remove_timer(Some(&frame_timer), kCFRunLoopDefaultMode);
                    }
                }

                // Deregister NSView from NotificationCenter.
                let notification_center = NSNotificationCenter::defaultCenter();
                notification_center.removeObserver(&ns_view);

                drop(window_state);

                // Close the window if in non-parented mode
                if let Some(ns_window) = self.ns_window.take() {
                    ns_window.close();
                }

                // Ensure that the NSView is detached from the parent window
                ns_view.removeFromSuperview();
                drop(ns_view);

                // If in non-parented mode, we want to also quit the app altogether
                let app = self.ns_app.take();
                if let Some(app) = app {
                    app.stop(Some(&app));
                }
            }
        }
    }

    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = AppKitWindowHandle::empty();

        if self.open.get() {
            let ns_window = self.ns_window.get().or(self.parent_ns_window.get());

            handle.ns_window = match ns_window {
                None => ptr::null_mut(),
                Some(view) => (&*view as *const NSWindow) as *mut _,
            };

            handle.ns_view = match self.ns_view.get() {
                None => ptr::null_mut(),
                Some(view) => (&*view as *const NSView) as *mut _,
            };
        }

        handle.into()
    }
}

pub struct Window<'a> {
    inner: &'a WindowInner,
}

impl<'a> Window<'a> {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let scaling = match options.scale {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        };

        let window_info = WindowInfo::from_logical_size(options.size, scaling);

        let handle = if let RawWindowHandle::AppKit(handle) = parent.raw_window_handle() {
            handle
        } else {
            panic!("Not a macOS window");
        };

        let ns_view = create_view(&options);
        let parent_window = unsafe { Retained::retain(handle.ns_window as *mut NSWindow) };
        let parent_view = unsafe { Retained::retain(handle.ns_view as *mut NSView) };

        let window_inner = WindowInner {
            open: Cell::new(true),
            ns_app: RetainedCell::empty(),
            ns_window: RetainedCell::empty(),
            parent_ns_window: RetainedCell::with(parent_window.clone()),
            ns_view: RetainedCell::new(ns_view.clone()),

            #[cfg(feature = "opengl")]
            gl_context: options
                .gl_config
                .map(|gl_config| Self::create_gl_context(None, &ns_view, gl_config)),
        };

        let window_handle = Self::init(window_inner, window_info, build);

        if let Some(parent_view) = parent_view {
            parent_view.addSubview(&ns_view);
        }

        window_handle
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let Some(mtm) = MainThreadMarker::new() else {
            panic!("macOS: open_blocking can only be called on the main thread!")
        };

        // Creates the global NSApplication instance, if it doesn't exist yet
        let app = NSApplication::sharedApplication(mtm);

        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        let scaling = match options.scale {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        };

        let rect = NSRect::new(
            NSPoint::ZERO,
            NSSize { width: options.size.width, height: options.size.height },
        );

        let window_info = WindowInfo::from_logical_size(options.size, scaling);

        // SAFETY: TODO
        let ns_window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        // SAFETY: TODO
        unsafe { ns_window.setReleasedWhenClosed(false) };

        ns_window.center();

        let title = NSString::from_str(&options.title);
        ns_window.setTitle(&title);

        ns_window.makeKeyAndOrderFront(None);

        let ns_view = create_view(&options);
        let window_inner = WindowInner {
            open: Cell::new(true),
            ns_app: RetainedCell::new(app.clone()),
            parent_ns_window: RetainedCell::empty(),
            ns_view: RetainedCell::new(ns_view.clone()),

            #[cfg(feature = "opengl")]
            gl_context: options
                .gl_config
                .map(|gl_config| Self::create_gl_context(Some(&ns_window), &ns_view, gl_config)),

            ns_window: RetainedCell::new(ns_window.clone()),
        };

        let _ = Self::init(window_inner, window_info, build);

        ns_window.setContentView(Some(&ns_view));
        let () = unsafe { msg_send![&*ns_window, setDelegate: &*ns_view] };

        app.run();
    }

    fn init<H, B>(window_inner: WindowInner, window_info: WindowInfo, build: B) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let mut window = crate::Window::new(Window { inner: &window_inner });
        let window_handler = Box::new(build(&mut window));

        let ns_view = window_inner.ns_view.get().unwrap();

        let window_state = Rc::new(WindowState {
            window_inner,
            window_handler: RefCell::new(window_handler),
            keyboard_state: KeyboardState::new(),
            frame_timer: RetainedCell::empty(),
            window_info: Cell::new(window_info),
            deferred_events: RefCell::default(),
        });

        let window_state_ptr = Rc::into_raw(Rc::clone(&window_state));

        unsafe {
            // TODO: Pretty certain this is a cyclic reference (aaaa)
            ns_view
                .class()
                .instance_variable(BASEVIEW_STATE_IVAR)
                .unwrap()
                .load_ptr::<*const c_void>(&ns_view)
                .write(window_state_ptr as *const c_void);

            WindowState::setup_timer(window_state_ptr);
        }

        WindowHandle { state: window_state }
    }

    pub fn close(&mut self) {
        self.inner.close();
    }

    pub fn has_focus(&mut self) -> bool {
        let view = self.inner.ns_view.get().unwrap();
        let Some(window) = view.window() else {
            return false;
        };

        if !window.isKeyWindow() {
            return false;
        }

        let Some(first_responder) = window.firstResponder() else {
            return false;
        };

        view.isEqual(Some(&*first_responder))
    }

    pub fn focus(&mut self) {
        let view = self.inner.ns_view.get().unwrap();
        if let Some(window) = view.window() {
            window.makeFirstResponder(Some(&view));
        }
    }

    pub fn resize(&mut self, size: Size) {
        if self.inner.open.get() {
            // NOTE: macOS gives you a personal rave if you pass in fractional pixels here. Even
            // though the size is in fractional pixels.
            let size = NSSize::new(size.width.round(), size.height.round());

            if let Some(view) = self.inner.ns_view.get() {
                view.setFrameSize(size);
                view.setNeedsDisplay(true);
            }

            // When using OpenGL the `NSOpenGLView` needs to be resized separately? Why? Because
            // macOS.
            #[cfg(feature = "opengl")]
            if let Some(gl_context) = &self.inner.gl_context {
                gl_context.resize(size);
            }

            // If this is a standalone window then we'll also need to resize the window itself
            if let Some(ns_window) = self.inner.ns_window.get() {
                ns_window.setContentSize(size);
            }
        }
    }

    pub fn set_mouse_cursor(&mut self, _mouse_cursor: MouseCursor) {
        todo!()
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&GlContext> {
        self.inner.gl_context.as_ref()
    }

    #[cfg(feature = "opengl")]
    fn create_gl_context(
        ns_window: Option<&NSWindow>, ns_view: &NSView, config: GlConfig,
    ) -> GlContext {
        let mut handle = AppKitWindowHandle::empty();
        handle.ns_window = match ns_window {
            Some(ns_window) => ns_window as *const NSWindow as *mut c_void,
            None => ptr::null_mut(),
        };
        handle.ns_view = ns_view as *const NSView as *mut c_void;
        let handle = RawWindowHandle::AppKit(handle);

        unsafe { GlContext::create(&handle, config).expect("Could not create OpenGL context") }
    }
}

pub(super) struct WindowState {
    pub(super) window_inner: WindowInner,
    window_handler: RefCell<Box<dyn WindowHandler>>,
    keyboard_state: KeyboardState,
    frame_timer: RetainedCell<CFRunLoopTimer>,
    /// The last known window info for this window.
    pub window_info: Cell<WindowInfo>,

    /// Events that will be triggered at the end of `window_handler`'s borrow.
    deferred_events: RefCell<VecDeque<Event>>,
}

impl WindowState {
    /// Gets the `WindowState` held by a given `NSView`.
    ///
    /// This method returns a cloned `Rc<WindowState>` rather than just a `&WindowState`, since the
    /// original `Rc<WindowState>` owned by the `NSView` can be dropped at any time
    /// (including during an event handler).
    ///
    /// # Safety
    ///
    /// `view` MUST be our own NSView, as created by `create_view`
    pub(super) unsafe fn from_view(view: &NSView) -> Rc<WindowState> {
        let state_ptr = view
            .class()
            .instance_variable(BASEVIEW_STATE_IVAR)
            .unwrap()
            .load::<*const c_void>(view)
            .cast::<WindowState>();

        let state_rc = Rc::from_raw(state_ptr);
        let state = Rc::clone(&state_rc);
        let _ = Rc::into_raw(state_rc);

        state
    }

    /// Trigger the event immediately and return the event status.
    /// Will panic if `window_handler` is already borrowed (see `trigger_deferrable_event`).
    pub(super) fn trigger_event(&self, event: Event) -> EventStatus {
        let mut window = crate::Window::new(Window { inner: &self.window_inner });
        let mut window_handler = self.window_handler.borrow_mut();
        let status = window_handler.on_event(&mut window, event);
        self.send_deferred_events(window_handler.as_mut());
        status
    }

    /// Trigger the event immediately if `window_handler` can be borrowed mutably,
    /// otherwise add the event to a queue that will be cleared once `window_handler`'s mutable borrow ends.
    /// As this method might result in the event triggering asynchronously, it can't reliably return the event status.
    pub(super) fn trigger_deferrable_event(&self, event: Event) {
        if let Ok(mut window_handler) = self.window_handler.try_borrow_mut() {
            let mut window = crate::Window::new(Window { inner: &self.window_inner });
            window_handler.on_event(&mut window, event);
            self.send_deferred_events(window_handler.as_mut());
        } else {
            self.deferred_events.borrow_mut().push_back(event);
        }
    }

    pub(super) fn trigger_frame(&self) {
        let mut window = crate::Window::new(Window { inner: &self.window_inner });
        let mut window_handler = self.window_handler.borrow_mut();
        window_handler.on_frame(&mut window);
        self.send_deferred_events(window_handler.as_mut());
    }

    pub(super) fn keyboard_state(&self) -> &KeyboardState {
        &self.keyboard_state
    }

    pub(super) fn process_native_key_event(&self, event: &NSEvent) -> Option<KeyboardEvent> {
        self.keyboard_state.process_native_event(event)
    }

    unsafe fn setup_timer(window_state_ptr: *const WindowState) {
        unsafe extern "C-unwind" fn timer_callback(
            _: *mut CFRunLoopTimer, window_state_ptr: *mut c_void,
        ) {
            unsafe {
                let window_state = &*(window_state_ptr as *const WindowState);

                window_state.trigger_frame();
            }
        }

        let Some(current_loop) = CFRunLoop::current() else {
            return;
        };

        let mut timer_context = CFRunLoopTimerContext {
            version: 0,
            info: window_state_ptr as *mut c_void,
            retain: None,
            release: None,
            copyDescription: None,
        };

        let Some(timer) = CFRunLoopTimer::new(
            kCFAllocatorDefault,
            0.0,
            0.015,
            0,
            0,
            Some(timer_callback),
            &mut timer_context,
        ) else {
            return;
        };

        current_loop.add_timer(Some(&timer), kCFRunLoopDefaultMode);

        (*window_state_ptr).frame_timer.set(timer.into());
    }

    fn send_deferred_events(&self, window_handler: &mut dyn WindowHandler) {
        let mut window = crate::Window::new(Window { inner: &self.window_inner });
        loop {
            let next_event = self.deferred_events.borrow_mut().pop_front();
            if let Some(event) = next_event {
                window_handler.on_event(&mut window, event);
            } else {
                break;
            }
        }
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.inner.raw_window_handle()
    }
}

unsafe impl<'a> HasRawDisplayHandle for Window<'a> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::AppKit(AppKitDisplayHandle::empty())
    }
}

pub fn copy_to_clipboard(string: &str) {
    let pb = NSPasteboard::generalPasteboard();
    let ns_str = NSString::from_str(string);

    pb.clearContents();
    pb.setString_forType(&ns_str, unsafe { NSPasteboardTypeString });
}
