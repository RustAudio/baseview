#![allow(deprecated)] // Allow use of NSFilenamesPboardType for now

use super::keyboard::{make_modifiers, KeyboardState};
use super::window::WindowSharedState;
use crate::wrappers::appkit::*;
use crate::MouseEvent::{ButtonPressed, ButtonReleased};
use crate::{
    DropData, DropEffect, Event, EventStatus, MouseButton, MouseEvent, Point, ScrollDelta, Size,
    WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions,
};
use objc2::__framework_prelude::Retained;
use objc2::rc::Weak;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{msg_send, AllocAnyThread};
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingInfo, NSEvent, NSFilenamesPboardType, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindow,
};
use objc2_foundation::{NSArray, NSNotification, NSPoint, NSRect, NSSize, NSString};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub enum ViewParentingType {
    Parented { parent_view: Weak<NSView> },
    Windowed { owned_window: Weak<NSWindow>, running_app: Weak<NSApplication> },
}

pub(crate) struct BaseviewView {
    pub(crate) state: Rc<WindowSharedState>,
    window_handler: RefCell<Option<Box<dyn WindowHandler>>>,

    frame_timer: Cell<Option<TimerHandle>>,
    notification_center_observer: Cell<Option<NotificationCenterObserver>>,

    keyboard_state: KeyboardState,

    parenting: ViewParentingType,

    #[cfg(feature = "opengl")]
    pub(crate) gl_context: std::cell::OnceCell<crate::gl::GlContext>,
}

impl BaseviewView {
    pub fn new<H: WindowHandler + 'static>(
        options: WindowOpenOptions, builder: impl FnOnce(&mut crate::Window) -> H,
        parenting: ViewParentingType,
    ) -> (Retained<View<Self>>, Rc<WindowSharedState>) {
        let view_rect =
            NSRect::new(NSPoint::ZERO, NSSize::new(options.size.width, options.size.height));

        let state = Rc::new(WindowSharedState::new(&options));

        let inner = BaseviewView {
            state: state.clone(),

            keyboard_state: KeyboardState::new(),
            frame_timer: None.into(),
            window_handler: None.into(),
            notification_center_observer: None.into(),
            parenting,

            #[cfg(feature = "opengl")]
            gl_context: std::cell::OnceCell::new(),
        };

        let view = View::new(view_rect, inner, |view| {
            // Set up parenting before handler setup
            match &view.parenting {
                ViewParentingType::Parented { parent_view } => {
                    let parent_view = parent_view.load().unwrap();
                    parent_view.addSubview(view.view);
                }
                ViewParentingType::Windowed { owned_window, .. } => {
                    let owned_window = owned_window.load().unwrap();
                    owned_window.setContentView(Some(view.view));
                    set_delegate(&owned_window, view.view);
                }
            }

            #[cfg(feature = "opengl")]
            if let Some(gl_config) = options.gl_config {
                let gl_context = super::gl::GlContext::create(view.view, gl_config).unwrap();
                let gl_context = crate::gl::GlContext::new(gl_context);
                let Ok(()) = view.gl_context.set(gl_context) else { unreachable!() };
            }

            // Initialize handler
            view.window_handler.replace(Some(Box::new(builder(&mut view.into()))));

            // Set up anything that might trigger events to the handler

            // SAFETY: This static is a read-only constant
            let ns_filenames_pboard_type = unsafe { NSFilenamesPboardType };
            view.view.registerForDraggedTypes(&NSArray::from_slice(&[ns_filenames_pboard_type]));

            let timer_view = Weak::new(view.view);
            view.frame_timer.set(TimerHandle::new(0.015, move || {
                if let Some(view) = timer_view.load() {
                    Self::trigger_frame(view.inner_ref());
                }
            }));

            let notifier_view = Weak::new(view.view);
            let observer = NotificationCenterObserver::register_window_key_change(move |n| {
                if let Some(view) = notifier_view.load() {
                    BaseviewView::handle_notification(view.inner_ref(), n);
                }
            });
            view.notification_center_observer.set(Some(observer));

            // Send an initial Resized event so users get the correct scale factor and physical size.
            Self::trigger_event(
                view,
                Event::Window(WindowEvent::Resized(Self::fetch_view_size(view.view))),
            );
        });

        (view, state)
    }

    pub fn close(this: ViewRef<Self>) {
        this.state.closed.set(true);
        this.view.removeFromSuperview();

        if let ViewParentingType::Windowed { owned_window: parent_window, running_app } =
            &this.parenting
        {
            if let Some(parent_window) = parent_window.load() {
                parent_window.close();
            }

            if let Some(app) = running_app.load() {
                app.stop(Some(&app));
            }
        }
    }

    pub fn resize(this: ViewRef<Self>, size: Size) {
        // NOTE: macOS gives you a personal rave if you pass in fractional pixels here. Even
        // though the size is in fractional pixels.
        let size = NSSize::new(size.width.round(), size.height.round());

        this.view.setFrameSize(size);
        this.view.setNeedsDisplay(true);

        // When using OpenGL the `NSOpenGLView` needs to be resized separately? Why? Because
        // macOS.
        #[cfg(feature = "opengl")]
        if let Some(gl_context) = this.gl_context.get() {
            gl_context.inner.resize(size);
        }

        // If this is a standalone window then we'll also need to resize the window itself
        if let ViewParentingType::Windowed { owned_window, .. } = &this.parenting {
            if let Some(owned_window) = owned_window.load() {
                owned_window.setContentSize(size);
            }
        }

        Self::view_did_change_backing_properties(this);
    }

    /// Trigger the event immediately and return the event status.
    fn trigger_event(this: ViewRef<Self>, event: Event) -> EventStatus {
        let handler = this.window_handler.borrow();
        let Some(handler) = handler.as_ref() else {
            return EventStatus::Ignored;
        };

        handler.on_event(&mut this.into(), event)
    }

    fn trigger_frame(this: ViewRef<Self>) {
        let handler = this.window_handler.borrow();
        let Some(handler) = handler.as_ref() else { return };

        handler.on_frame(&mut this.into());
    }

    fn fetch_view_size(view: &NSView) -> WindowInfo {
        let ns_window = view.window();

        let scale_factor: f64 = ns_window.map(|w| w.backingScaleFactor()).unwrap_or(1.0);

        let bounds = view.bounds();

        WindowInfo::from_logical_size(
            Size::new(bounds.size.width, bounds.size.height),
            scale_factor,
        )
    }
}

impl Drop for BaseviewView {
    fn drop(&mut self) {
        self.state.closed.set(true);
    }
}

impl ViewImpl for BaseviewView {
    fn become_first_responder(this: ViewRef<Self>) -> bool {
        let Some(window) = this.view.window() else {
            return true;
        };

        if window.isKeyWindow() {
            Self::trigger_event(this, Event::Window(WindowEvent::Focused));
        }

        true
    }

    fn resign_first_responder(this: ViewRef<Self>) -> bool {
        Self::trigger_event(this, Event::Window(WindowEvent::Unfocused));
        true
    }

    fn window_should_close(this: ViewRef<Self>) -> bool {
        Self::trigger_event(this, Event::Window(WindowEvent::WillClose));
        Self::close(this);

        true
    }

    fn view_did_change_backing_properties(this: ViewRef<Self>) {
        let new_window_info = Self::fetch_view_size(this.view);
        let window_info = this.state.window_info.get();

        // Only send the event when the window's size has actually changed to be in line with the
        // other platform implementations
        if new_window_info.physical_size() != window_info.physical_size() {
            this.state.window_info.set(new_window_info);
            Self::trigger_event(this, Event::Window(WindowEvent::Resized(new_window_info)));
        }
    }

    /// `hitTest:` override that collapses hits on baseview's internal
    /// OpenGL render subview to this NSView.
    ///
    /// `src/gl/gl` attaches an `NSOpenGLView` as a subview of this
    /// view so the GL context is isolated from event handling. The side
    /// effect is that `[NSView hitTest:]` returns the GL subview for
    /// every click inside our frame — `NSOpenGLView` inherits the
    /// default `acceptsFirstMouse:` which returns `NO`, so AppKit treats
    /// the first click in a non-key window as an activation click and
    /// never dispatches `mouseDown:`. That's the "first click dead zone"
    /// symptom reported in baseview#129 / #202 / #169.
    ///
    /// Fix: if the hit lands on our own GL render subview (pointer
    /// equality against the `NSOpenGLView` stored in `GlContext`),
    /// collapse the result to `self`. AppKit then asks US about
    /// `acceptsFirstMouse:` (we return `YES`), and `mouseDown:` is
    /// dispatched on the first click. Hits on any other subview pass
    /// through unchanged — we only redirect our own render child, not
    /// anything the consumer may add.
    ///
    /// No-op without the `opengl` feature: there's no GL subview to
    /// collapse, so the override pass-through is equivalent to the
    /// default implementation.
    fn hit_test(this: ViewRef<'_, Self>, point: NSPoint) -> Option<&NSView> {
        let superclass = this.view.class().superclass().unwrap();

        // SAFETY: Our superclass is NSView
        let super_result: Option<&NSView> =
            unsafe { msg_send![super(this.view, superclass), hitTest: point] };
        let super_result = super_result?;

        #[cfg(feature = "opengl")]
        {
            if let Some(gl_context) = this.gl_context.get() {
                if *super_result == **gl_context.inner.0.view {
                    return Some(this.view);
                }
            }
        }

        Some(super_result)
    }

    fn view_will_move_to_window(this: ViewRef<Self>, new_window: Option<&NSWindow>) {
        let tracking_areas = this.view.trackingAreas();

        match new_window {
            None => {
                if tracking_areas.count() > 0 {
                    let tracking_area = tracking_areas.objectAtIndex(0);
                    this.view.removeTrackingArea(&tracking_area);
                }
            }
            Some(new_window) => {
                if tracking_areas.is_empty() {
                    let tracking_area = new_tracking_area(this.view);
                    this.view.addTrackingArea(&tracking_area);
                }

                new_window.setAcceptsMouseMovedEvents(true);
                new_window.makeFirstResponder(Some(this.view));
            }
        }

        unsafe {
            let superclass = msg_send![this.view, superclass];

            let () = msg_send![super(this.view, superclass), viewWillMoveToWindow: new_window];
        }
    }

    fn update_tracking_areas(this: ViewRef<Self>) {
        let tracking_areas = this.view.trackingAreas();
        if tracking_areas.count() > 0 {
            let tracking_area = tracking_areas.objectAtIndex(0);
            this.view.removeTrackingArea(&tracking_area);
        }

        let tracking_area = new_tracking_area(this.view);

        this.view.addTrackingArea(&tracking_area);
    }

    fn mouse_moved(this: ViewRef<Self>, event: &NSEvent) {
        let point = this.view.convertPoint_fromView(event.locationInWindow(), None);

        let position = Point { x: point.x, y: point.y };

        Self::trigger_event(
            this,
            Event::Mouse(MouseEvent::CursorMoved {
                position,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn scroll_wheel(this: ViewRef<Self>, event: &NSEvent) {
        let x = event.scrollingDeltaX() as f32;
        let y = event.scrollingDeltaY() as f32;

        let delta = if event.hasPreciseScrollingDeltas() {
            ScrollDelta::Pixels { x, y }
        } else {
            ScrollDelta::Lines { x, y }
        };

        Self::trigger_event(
            this,
            Event::Mouse(MouseEvent::WheelScrolled {
                delta,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn dragging_entered(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> NSDragOperation {
        let modifiers = this.keyboard_state.last_mods();
        let drop_data = get_drop_data(sender);

        let event = MouseEvent::DragEntered {
            position: get_drag_position(sender),
            modifiers: make_modifiers(modifiers),
            data: drop_data,
        };

        on_event(this, event)
    }

    fn dragging_updated(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> NSDragOperation {
        let modifiers = this.keyboard_state.last_mods();
        let drop_data = get_drop_data(sender);

        let event = MouseEvent::DragMoved {
            position: get_drag_position(sender),
            modifiers: make_modifiers(modifiers),
            data: drop_data,
        };

        on_event(this, event)
    }

    fn prepare_for_drag_operation(
        _this: ViewRef<Self>, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> bool {
        // Always accept drag operation if we get this far
        // This function won't be called unless dragging_entered/updated
        // has returned an acceptable operation
        true
    }

    fn perform_drag_operation(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> bool {
        let modifiers = this.keyboard_state.last_mods();
        let drop_data = get_drop_data(sender);

        let event = MouseEvent::DragDropped {
            position: get_drag_position(sender),
            modifiers: make_modifiers(modifiers),
            data: drop_data,
        };

        let event_status = Self::trigger_event(this, Event::Mouse(event));

        matches!(event_status, EventStatus::AcceptDrop(_))
    }

    fn dragging_exited(this: ViewRef<Self>, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
        on_event(this, MouseEvent::DragLeft);
    }

    fn handle_notification(this: ViewRef<Self>, notification: &NSNotification) {
        let Some(window) = this.view.window() else { return };
        // The subject of the notification, in this case an NSWindow object.
        let Some(notification_object) = notification.object().and_then(|o| o.downcast().ok())
        else {
            return;
        };

        // Only trigger focus events if the NSWindow that's being notified about is our window,
        // and if the window's first responder is our NSView.
        if window != notification_object {
            return;
        }

        let Some(first_responder) = window.firstResponder() else { return };

        // If the first responder isn't our NSView, the focus events will instead be triggered
        // by the becomeFirstResponder and resignFirstResponder methods on the NSView itself.
        if !this.view.isEqual(Some(&first_responder)) {
            return;
        }

        Self::trigger_event(
            this,
            Event::Window(if window.isKeyWindow() {
                WindowEvent::Focused
            } else {
                WindowEvent::Unfocused
            }),
        );
    }

    fn mouse_down(this: ViewRef<Self>, event: &NSEvent) {
        Self::trigger_event(
            this,
            Event::Mouse(ButtonPressed {
                button: MouseButton::Left,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn mouse_up(this: ViewRef<Self>, event: &NSEvent) {
        Self::trigger_event(
            this,
            Event::Mouse(ButtonReleased {
                button: MouseButton::Left,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn right_mouse_down(this: ViewRef<Self>, event: &NSEvent) {
        Self::trigger_event(
            this,
            Event::Mouse(ButtonPressed {
                button: MouseButton::Right,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn right_mouse_up(this: ViewRef<Self>, event: &NSEvent) {
        Self::trigger_event(
            this,
            Event::Mouse(ButtonReleased {
                button: MouseButton::Right,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn other_mouse_down(this: ViewRef<Self>, event: &NSEvent) {
        Self::trigger_event(
            this,
            Event::Mouse(ButtonPressed {
                button: MouseButton::Middle,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn other_mouse_up(this: ViewRef<Self>, event: &NSEvent) {
        Self::trigger_event(
            this,
            Event::Mouse(ButtonReleased {
                button: MouseButton::Middle,
                modifiers: make_modifiers(event.modifierFlags()),
            }),
        );
    }

    fn mouse_entered(this: ViewRef<Self>) {
        Self::trigger_event(this, Event::Mouse(MouseEvent::CursorEntered));
    }

    fn mouse_exited(this: ViewRef<Self>) {
        Self::trigger_event(this, Event::Mouse(MouseEvent::CursorLeft));
    }

    fn key_down(this: ViewRef<Self>, event: &NSEvent) {
        if let Some(key_event) = this.keyboard_state.process_native_event(event) {
            let status = Self::trigger_event(this, Event::Keyboard(key_event));

            if let EventStatus::Ignored = status {
                unsafe {
                    let superclass = msg_send![this.view, superclass];

                    let () = msg_send![super(this.view, superclass), keyDown:event];
                }
            }
        }
    }

    fn key_up(this: ViewRef<Self>, event: &NSEvent) {
        if let Some(key_event) = this.keyboard_state.process_native_event(event) {
            let status = Self::trigger_event(this, Event::Keyboard(key_event));

            if let EventStatus::Ignored = status {
                unsafe {
                    let superclass = msg_send![this.view, superclass];

                    let () = msg_send![super(this.view, superclass), keyUp:event];
                }
            }
        }
    }

    fn flags_changed(this: ViewRef<Self>, event: &NSEvent) {
        if let Some(key_event) = this.keyboard_state.process_native_event(event) {
            let status = Self::trigger_event(this, Event::Keyboard(key_event));

            if let EventStatus::Ignored = status {
                unsafe {
                    let superclass = msg_send![this.view, superclass];

                    let () = msg_send![super(this.view, superclass), flagsChanged:event];
                }
            }
        }
    }
}

/// Info:
/// https://developer.apple.com/documentation/appkit/nstrackingarea
/// https://developer.apple.com/documentation/appkit/nstrackingarea/options
/// https://developer.apple.com/documentation/appkit/nstrackingareaoptions
fn new_tracking_area(this: &NSView) -> Retained<NSTrackingArea> {
    let options = NSTrackingAreaOptions::MouseEnteredAndExited
        | NSTrackingAreaOptions::MouseMoved
        | NSTrackingAreaOptions::CursorUpdate
        | NSTrackingAreaOptions::ActiveInActiveApp
        | NSTrackingAreaOptions::InVisibleRect
        | NSTrackingAreaOptions::EnabledDuringMouseDrag;

    // SAFETY: `this` is of the correct type (NSView)
    unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            NSTrackingArea::alloc(),
            this.bounds(),
            options,
            Some(this),
            None,
        )
    }
}

fn get_drag_position(sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) -> Point {
    let point = match sender {
        Some(sender) => sender.draggingLocation(),
        None => NSPoint::ZERO,
    };

    Point::new(point.x, point.y)
}

fn get_drop_data(sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) -> DropData {
    let Some(sender) = sender else {
        return DropData::None;
    };

    let pasteboard = sender.draggingPasteboard();
    let Some(file_list) = pasteboard.propertyListForType(unsafe { NSFilenamesPboardType }) else {
        return DropData::None;
    };

    let Ok(file_list) = file_list.downcast::<NSArray>() else {
        return DropData::None;
    };

    let files = file_list
        .into_iter()
        .filter_map(|s| s.downcast::<NSString>().ok())
        .map(|s| s.to_string().into())
        .collect();

    DropData::Files(files)
}

fn on_event(this: ViewRef<BaseviewView>, event: MouseEvent) -> NSDragOperation {
    let event_status = BaseviewView::trigger_event(this, Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(DropEffect::Copy) => NSDragOperation::Copy,
        EventStatus::AcceptDrop(DropEffect::Move) => NSDragOperation::Move,
        EventStatus::AcceptDrop(DropEffect::Link) => NSDragOperation::Link,
        EventStatus::AcceptDrop(DropEffect::Scroll) => NSDragOperation::Generic,
        _ => NSDragOperation::None,
    }
}
