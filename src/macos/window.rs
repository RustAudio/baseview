use std::cell::Cell;
use std::rc::Rc;

use objc2::rc::{autoreleasepool, Weak};
use objc2::runtime::NSObjectProtocol;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSPasteboard, NSPasteboardTypeString,
};
use objc2_foundation::NSString;
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use crate::{MouseCursor, Size, WindowHandler, WindowInfo, WindowOpenOptions};

#[cfg(feature = "opengl")]
use crate::gl::GlContext;
use crate::macos::view::BaseviewView;
use crate::wrappers::appkit::{
    create_window, extract_raw_window_handle, set_delegate, View, ViewRef,
};

pub struct WindowHandle {
    view: Option<Weak<View<BaseviewView>>>,
    state: Rc<WindowSharedState>,
}

impl WindowHandle {
    pub fn close(&mut self) {
        let Some(view) = self.view.take().and_then(|w| w.load()) else {
            return;
        };

        BaseviewView::close(view.inner_ref());
    }

    pub fn is_open(&self) -> bool {
        self.state.closed.get()
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let Some(view) = self.view.as_ref().and_then(|w| w.load()) else {
            return AppKitWindowHandle::empty().into();
        };

        view.raw_window_handle()
    }
}

pub struct Window<'a> {
    view: &'a View<BaseviewView>,
    inner: &'a BaseviewView,
}

impl<'a> From<ViewRef<'a, BaseviewView>> for crate::Window<'a> {
    fn from(value: ViewRef<'a, BaseviewView>) -> Self {
        crate::Window::new(Window { view: value.view, inner: value.inner })
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
            let (_parent_window, parent_view) =
                extract_raw_window_handle(parent.raw_window_handle());

            let (ns_view, state) = BaseviewView::new(options, build, None);

            if let Some(parent_view) = parent_view {
                parent_view.addSubview(&ns_view);
            }

            WindowHandle { view: Some(Weak::from_retained(&ns_view)), state }
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

            let window = create_window(options.size, mtm);
            window.center();

            let title = NSString::from_str(&options.title);
            window.setTitle(&title);
            window.makeKeyAndOrderFront(None);

            let (view, _) = BaseviewView::new(options, build, Some(Weak::from_retained(&app)));

            window.setContentView(Some(&view));
            set_delegate(&window, &view);

            app.run();
        })
    }

    pub fn close(&mut self) {
        BaseviewView::close(self.view.inner_ref());
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
        todo!()
        /*
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
        }*/
    }

    pub fn set_mouse_cursor(&mut self, _mouse_cursor: MouseCursor) {
        todo!()
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&GlContext> {
        self.inner.gl_context.get()
    }
}

pub(crate) struct WindowSharedState {
    /// The last known window info for this window.
    pub window_info: Cell<WindowInfo>,
    pub closed: Cell<bool>,
}

impl WindowSharedState {
    pub fn new(options: &WindowOpenOptions) -> Self {
        Self {
            window_info: WindowInfo::from_logical_size(options.size, 1.0).into(),
            closed: false.into(),
        }
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.view.raw_window_handle()
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
