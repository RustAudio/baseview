use dpi::{LogicalSize, PhysicalSize};
use objc2::rc::{autoreleasepool, Retained, Weak};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeString, NSView, NSWindow};
use objc2_foundation::{NSSize, NSString};
use std::cell::Cell;
use std::rc::Rc;

use crate::handler::WindowHandlerBuilder;
use crate::platform::macos::view::{BaseviewView, ViewParentingType};
use crate::platform::Result;
use crate::wrappers::appkit::{create_window, View};
use crate::*;

pub struct WindowHandle {
    mtm: MainThreadMarker,
    view: Weak<View<BaseviewView>>,
    _window: Option<Retained<NSWindow>>,
    state: Rc<WindowSharedState>,
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        let Some(view) = self.view.load() else { return };
        let Some(view) = view.inner_ref() else { return };

        BaseviewView::close(view);
    }
}

impl WindowHandle {
    pub fn create_window(
        mut options: WindowOpenOptions, handler: WindowHandlerBuilder,
    ) -> Result<Self> {
        autoreleasepool(|_| {
            let Some(mtm) = MainThreadMarker::new() else {
                panic!("macOS: Windows can only be created on the main thread!")
            };

            // Creates the global NSApplication instance, if it doesn't exist yet
            let _ = NSApplication::sharedApplication(mtm);

            if let Some(parent) = options.parent.take() {
                return Self::create_window_parented(options, handler, parent.view, mtm);
            }

            Self::create_window_standalone(options, handler, mtm)
        })
    }

    pub fn create_window_parented(
        builder: WindowOpenOptions, handler: WindowHandlerBuilder, parent_view: Retained<NSView>,
        mtm: MainThreadMarker,
    ) -> Result<Self> {
        let parenting =
            ViewParentingType::Parented { parent_view: Weak::from_retained(&parent_view) };

        let backing_scale_factor =
            parent_view.window().map(|w| w.backingScaleFactor()).unwrap_or(1.0);
        let final_size = builder.size.to_logical(backing_scale_factor);

        let (ns_view, state) = BaseviewView::new(builder, handler, parenting, final_size, mtm)?;

        Ok(Self { mtm, state, _window: None, view: Weak::from_retained(&ns_view) })
    }

    pub fn create_window_standalone(
        builder: WindowOpenOptions, handler: WindowHandlerBuilder, mtm: MainThreadMarker,
    ) -> Result<Self> {
        let app = NSApplication::sharedApplication(mtm);
        let window = create_window_with_options(&builder, mtm);

        let final_size = window.contentRectForFrameRect(window.frame()).size;
        let final_size = LogicalSize::new(final_size.width, final_size.height);

        let parenting = ViewParentingType::Windowed {
            running_app: Weak::from_retained(&app),
            owned_window: Weak::from_retained(&window),
        };

        let (view, state) = BaseviewView::new(builder, handler, parenting, final_size, mtm)?;

        Ok(Self { mtm, state, view: Weak::from_retained(&view), _window: Some(window) })
    }

    pub fn run_until_closed(self) -> Result<()> {
        NSApplication::sharedApplication(self.mtm).run();
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.state.closed.get()
    }

    pub fn size(&self) -> WindowSize {
        WindowSize::from_logical(self.state.size.get(), self.state.scale_factor.get())
    }
}

fn create_window_with_options(
    options: &WindowOpenOptions, mtm: MainThreadMarker,
) -> Retained<NSWindow> {
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
}

pub fn copy_to_clipboard(string: &str) {
    let pb = NSPasteboard::generalPasteboard();
    let ns_str = NSString::from_str(string);

    pb.clearContents();
    pb.setString_forType(&ns_str, unsafe { NSPasteboardTypeString });
}
