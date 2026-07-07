use dpi::LogicalSize;
use objc2::rc::{autoreleasepool, Retained, Weak};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeString, NSWindow};
use objc2_foundation::{NSSize, NSString};
use raw_window_handle::HasWindowHandle;
use std::cell::Cell;
use std::error::Error;
use std::rc::Rc;

use crate::platform::macos::view::{BaseviewView, ViewParentingType};
use crate::wrappers::appkit::{create_window, extract_raw_window_handle, View};
use crate::{WindowBuilder, WindowContext, WindowHandler, WindowOpenOptions};

enum WindowView {
    Uninitialized {
        initializer: Box<dyn FnOnce(WindowContext) -> Box<dyn WindowHandler>>,
        builder: WindowBuilder,
    },
    Initialized {
        inner: Weak<View<BaseviewView>>,
        window: Option<Retained<NSWindow>>,
    },
    InitializationFailed,
}

impl WindowView {
    fn load(&self) -> Option<Retained<View<BaseviewView>>> {
        match &self {
            WindowView::Initialized { inner, .. } => inner.load(),
            _ => None,
        }
    }
}

pub struct Window {
    view: WindowView,
    state: Rc<WindowSharedState>,
    mtm: MainThreadMarker,
}

impl Window {
    pub fn create_window<H: WindowHandler>(
        mut builder: WindowBuilder, handler: impl FnOnce(WindowContext) -> H + 'static,
    ) -> Self {
        autoreleasepool(|_| {
            let Some(mtm) = MainThreadMarker::new() else {
                panic!("macOS: Windows can only be created on the main thread!")
            };

            // Creates the global NSApplication instance, if it doesn't exist yet
            let _ = NSApplication::sharedApplication(mtm);

            if let Some(parent) = builder.parent.take() {
                return Self::create_window_parented(builder, handler, parent, mtm);
            }

            if !builder.parented {
                return Self::create_window_standalone(builder, handler, mtm);
            }

            // Delay window creation until parent is known
            Self {
                mtm,
                state: Rc::new(WindowSharedState::new_uninitialized(&builder)),
                view: WindowView::Uninitialized {
                    initializer: Box::new(|c| Box::new(handler(c))),
                    builder,
                },
            }
        })
    }

    pub fn create_window_parented<H: WindowHandler>(
        builder: WindowBuilder, handler: impl FnOnce(WindowContext) -> H + 'static,
        parent: Box<dyn HasWindowHandle>, mtm: MainThreadMarker,
    ) -> Self {
        let parent_view = extract_raw_window_handle(parent.window_handle().unwrap()).unwrap();

        let parenting =
            ViewParentingType::Parented { parent_view: Weak::from_retained(&parent_view) };

        let backing_scale_factor =
            parent_view.window().map(|w| w.backingScaleFactor()).unwrap_or(1.0);
        let final_size = builder.size.to_logical(backing_scale_factor);

        let (ns_view, state) = BaseviewView::new(builder, handler, parenting, final_size, mtm);

        Self {
            mtm,
            state,
            view: WindowView::Initialized { window: None, inner: Weak::from_retained(&ns_view) },
        }
    }

    pub fn create_window_standalone<H: WindowHandler>(
        builder: WindowBuilder, handler: impl FnOnce(WindowContext) -> H + 'static,
        mtm: MainThreadMarker,
    ) -> Self {
        let app = NSApplication::sharedApplication(mtm);
        let window = create_window_with_options(&builder, mtm);

        let final_size = window.contentRectForFrameRect(window.frame()).size;
        let final_size = LogicalSize::new(final_size.width, final_size.height);

        let parenting = ViewParentingType::Windowed {
            running_app: Weak::from_retained(&app),
            owned_window: Weak::from_retained(&window),
        };

        let (view, state) = BaseviewView::new(builder, handler, parenting, final_size, mtm);

        Self {
            mtm,
            state,
            view: WindowView::Initialized {
                inner: Weak::from_retained(&view),
                window: Some(window),
            },
        }
    }

    pub fn run_until_closed(self) -> Result<(), Box<dyn Error>> {
        NSApplication::sharedApplication(self.mtm).run();

        Ok(())
    }

    pub fn destroy(self) {
        drop(self);
    }

    pub fn is_open(&self) -> bool {
        self.state.closed.get()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(view) = self.view.load() {
            BaseviewView::close(view.inner_ref())
        }
    }
}

fn create_window_with_options(
    options: &WindowBuilder, mtm: MainThreadMarker,
) -> Retained<NSWindow> {
    let initial_size = options.size.to_logical(1.0);
    let window = create_window(initial_size, mtm);
    window.center();

    let final_size = options.size.to_logical(window.backingScaleFactor());
    if final_size != initial_size {
        window.setContentSize(NSSize::new(final_size.width, final_size.height));
    }

    if let Some(title) = &options.title {
        let title = NSString::from_str(title);
        window.setTitle(&title);
    }

    window.makeKeyAndOrderFront(None);
    window
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

    pub fn new_uninitialized(builder: &WindowBuilder) -> Self {
        Self {
            closed: false.into(),
            size: builder.size.to_logical(1.0).into(),
            scale_factor: 1.0.into(),
        }
    }
}

pub fn copy_to_clipboard(string: &str) {
    let pb = NSPasteboard::generalPasteboard();
    let ns_str = NSString::from_str(string);

    pb.clearContents();
    pb.setString_forType(&ns_str, unsafe { NSPasteboardTypeString });
}
