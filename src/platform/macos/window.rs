use std::cell::{Cell, RefCell};
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
    HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use super::cursor::Cursor;
use crate::{MouseCursor, Size, WindowContext, WindowHandler, WindowInfo, WindowOpenOptions};

#[cfg(feature = "opengl")]
use crate::gl::GlContext;
use crate::platform::macos::view::{BaseviewView, ViewParentingType};
use crate::wrappers::appkit::{create_window, extract_raw_window_handle, View, ViewRef};

pub struct WindowHandle {
    view: RefCell<Option<Weak<View<BaseviewView>>>>,
    state: Rc<WindowSharedState>,
}

impl WindowHandle {
    pub fn close(&self) {
        let Some(view) = self.view.take().and_then(|w| w.load()) else {
            return;
        };

        BaseviewView::close(view.inner_ref());
    }

    pub fn is_open(&self) -> bool {
        self.state.closed.get()
    }
}

pub struct Window;

impl Window {
    pub fn open_parented<H: WindowHandler>(
        parent: &impl HasWindowHandle, options: WindowOpenOptions,
        build: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) -> WindowHandle {
        autoreleasepool(|_| {
            let Some(parent_view) = extract_raw_window_handle(parent.window_handle().unwrap())
            else {
                panic!("Invalid window handle: ns_view is NULL");
            };

            let parenting =
                ViewParentingType::Parented { parent_view: Weak::from_retained(&parent_view) };

            let (ns_view, state) = BaseviewView::new(options, build, parenting);

            WindowHandle { view: Some(Weak::from_retained(&ns_view)).into(), state }
        })
    }

    pub fn open_blocking<H: WindowHandler>(
        options: WindowOpenOptions, build: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) {
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

            let parenting = ViewParentingType::Windowed {
                running_app: Weak::from_retained(&app),
                owned_window: Weak::from_retained(&window),
            };

            let _ = BaseviewView::new(options, build, parenting);

            app.run();
        })
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

pub fn copy_to_clipboard(string: &str) {
    let pb = NSPasteboard::generalPasteboard();
    let ns_str = NSString::from_str(string);

    pb.clearContents();
    pb.setString_forType(&ns_str, unsafe { NSPasteboardTypeString });
}
