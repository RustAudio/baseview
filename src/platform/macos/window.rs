use dpi::LogicalSize;
use objc2::rc::{autoreleasepool, Weak};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSPasteboard, NSPasteboardTypeString,
};
use objc2_foundation::{NSSize, NSString};
use raw_window_handle::HasWindowHandle;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::platform::macos::view::{BaseviewView, ViewParentingType};
use crate::wrappers::appkit::{create_window, extract_raw_window_handle, View};
use crate::{WindowContext, WindowHandler, WindowOpenOptions};

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

            let Some(mtm) = MainThreadMarker::new() else {
                panic!("macOS: open_blocking can only be called on the main thread!")
            };

            let parenting =
                ViewParentingType::Parented { parent_view: Weak::from_retained(&parent_view) };

            let backing_scale_factor =
                parent_view.window().map(|w| w.backingScaleFactor()).unwrap_or(1.0);
            let final_size = options.size.to_logical(backing_scale_factor);

            let (ns_view, state) = BaseviewView::new(options, build, parenting, final_size, mtm);

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

            let initial_size = options.size.to_logical(1.0);
            let window = create_window(initial_size, mtm);
            window.center();

            let final_size = options.size.to_logical(window.backingScaleFactor());
            if final_size != initial_size {
                window.setContentSize(NSSize::new(final_size.width, final_size.height));
            }

            let title = NSString::from_str(&options.title);
            window.setTitle(&title);
            window.makeKeyAndOrderFront(None);

            let parenting = ViewParentingType::Windowed {
                running_app: Weak::from_retained(&app),
                owned_window: Weak::from_retained(&window),
            };

            let _ = BaseviewView::new(options, build, parenting, final_size, mtm);

            app.run();
        })
    }
}

pub(crate) struct WindowSharedState {
    pub closed: Cell<bool>,
    pub size: Cell<LogicalSize<f64>>,
    pub scale_factor: Cell<f64>,
}

impl WindowSharedState {
    pub fn new(size: LogicalSize<f64>, scale_factor: f64) -> Self {
        Self { closed: false.into(), size: size.into(), scale_factor: scale_factor.into() }
    }
}

pub fn copy_to_clipboard(string: &str) {
    let pb = NSPasteboard::generalPasteboard();
    let ns_str = NSString::from_str(string);

    pb.clearContents();
    pb.setString_forType(&ns_str, unsafe { NSPasteboardTypeString });
}
