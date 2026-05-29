use std::cell::Cell;
use std::ptr;
use std::rc::Rc;

use objc2::rc::{autoreleasepool, Retained, Weak};
use objc2::runtime::NSObjectProtocol;
use objc2::{msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSPasteboard,
    NSPasteboardTypeString, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use crate::{MouseCursor, Size, WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy};

#[cfg(feature = "opengl")]
use crate::gl::{GlConfig, GlContext};
use crate::macos::view::BaseviewView;
use crate::macos::RetainedCell;
use crate::wrappers::appkit::{View, ViewRef};

pub struct WindowHandle {
    view: Option<Weak<View<BaseviewView>>>,
    state: Rc<WindowState>,
}

impl WindowHandle {
    pub fn close(&mut self) {
        let Some(view) = self.view.take().and_then(|w| w.load()) else {
            return;
        };

        view.removeFromSuperview();

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
    ns_view: RetainedCell<View>,
}

impl WindowInner {
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
                Some(view) => (&*view as *const _) as *mut _,
            };
        }

        handle.into()
    }
}

pub struct Window<'a> {
    view: &'a View<BaseviewView>,
}

impl<'a> From<ViewRef<'a, BaseviewView>> for crate::Window<'a> {
    fn from(value: ViewRef<'a, BaseviewView>) -> Self {
        crate::Window::new(Window { view: value.view })
    }
}

impl<'a> Window<'a> {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        autoreleasepool(|_| {
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

            let ns_view = BaseviewView::new(options, build);
            let parent_window = unsafe { Retained::retain(handle.ns_window as *mut NSWindow) };
            let parent_view = unsafe { Retained::retain(handle.ns_view as *mut NSView) };

            let window_inner = WindowInner {
                open: Cell::new(true),
                ns_app: RetainedCell::empty(),
                ns_window: RetainedCell::empty(),
                parent_ns_window: RetainedCell::with(parent_window.clone()),
                ns_view: RetainedCell::new(ns_view.clone()),
            };

            let window_handle = Self::init(window_inner, window_info, build);

            if let Some(parent_view) = parent_view {
                parent_view.addSubview(&ns_view);
            }

            window_handle
        })
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        autoreleasepool(|_| {
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

            // SAFETY: This is safe because of the setReleasedWhenClosed(false) below
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

            // SAFETY: setReleasedWhenClosed is always safe to call with `false` (worst case is a memory leak)
            unsafe { ns_window.setReleasedWhenClosed(false) };

            ns_window.center();

            let title = NSString::from_str(&options.title);
            ns_window.setTitle(&title);

            ns_window.makeKeyAndOrderFront(None);

            let ns_view = BaseviewView::new(options, build);

            ns_window.setContentView(Some(&ns_view));
            let () = unsafe { msg_send![&*ns_window, setDelegate: &*ns_view] };

            app.run();
        })
    }

    pub fn close(&mut self) {
        self.inner.close();
    }

    pub fn has_focus(&mut self) -> bool {
        let Some(window) = self.view.window() else {
            return false;
        };

        if !window.isKeyWindow() {
            return false;
        }

        let Some(first_responder) = window.firstResponder() else {
            return false;
        };

        self.view.isEqual(Some(&*first_responder))
    }

    pub fn focus(&mut self) {
        if let Some(window) = self.view.window() {
            window.makeFirstResponder(Some(self.view));
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
    fn create_gl_context(ns_view: &NSView, config: GlConfig) -> GlContext {
        GlContext::create(ns_view, config).expect("Could not create OpenGL context")
    }
}

pub(super) struct WindowState {
    pub(super) window_inner: WindowInner,
    /// The last known window info for this window.
    pub window_info: Cell<WindowInfo>,
}

impl WindowState {
    pub fn new() -> Self {
        todo!()
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
