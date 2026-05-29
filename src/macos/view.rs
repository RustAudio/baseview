#![allow(deprecated)] // Allow use of NSFilenamesPboardType for now

use super::keyboard::{make_modifiers, KeyboardState};
use super::window::WindowState;
use crate::macos::Window;
use crate::wrappers::appkit::*;
use crate::MouseEvent::{ButtonPressed, ButtonReleased};
use crate::{
    DropData, DropEffect, Event, EventStatus, MouseButton, MouseEvent, Point, ScrollDelta, Size,
    WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions,
};
use objc2::__framework_prelude::Retained;
use objc2::rc::Weak;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{msg_send, sel, AllocAnyThread};
use objc2_app_kit::{
    NSDragOperation, NSDraggingInfo, NSEvent, NSFilenamesPboardType, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindow, NSWindowDidBecomeKeyNotification,
    NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSPoint, NSRect, NSSize, NSString,
};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

pub struct BaseviewView {
    state: Rc<WindowState>,
    window_handler: RefCell<Option<Box<dyn WindowHandler>>>,

    /// Events that will be triggered at the end of `window_handler`'s borrow.
    deferred_events: RefCell<VecDeque<Event>>,

    frame_timer: Cell<Option<TimerHandle>>,
    keyboard_state: KeyboardState,

    #[cfg(feature = "opengl")]
    gl_context: std::cell::OnceCell<crate::gl::GlContext>,
}
/*
pub(super) fn create_view<V: ViewImpl>(
    window_options: &WindowOpenOptions, inner: V,
) -> Retained<View<V>> {
    let size = window_options.size;
    let view = View::new(NSRect::new(NSPoint::ZERO, NSSize::new(size.width, size.height)), inner);

    /*
    let notification_center = NSNotificationCenter::defaultCenter();

    // SAFETY: Our NSView class does have a handleNotification: method with the matching signature.
    unsafe {
        notification_center.addObserver_selector_name_object(
            &view,
            sel!(handleNotification:),
            Some(NSWindowDidBecomeKeyNotification),
            None,
        );
        notification_center.addObserver_selector_name_object(
            &view,
            sel!(handleNotification:),
            Some(NSWindowDidResignKeyNotification),
            None,
        );
    }*/

    /*
    // SAFETY: This static is a read-only constant
    let ns_filenames_pboard_type = unsafe { NSFilenamesPboardType };
    view.registerForDraggedTypes(&NSArray::from_slice(&[ns_filenames_pboard_type]));

     */

    view
}*/

impl BaseviewView {
    pub fn new<H: WindowHandler + 'static>(
        options: WindowOpenOptions, builder: impl FnOnce(&mut crate::Window) -> H,
    ) -> Retained<View<Self>> {
        let view_rect =
            NSRect::new(NSPoint::ZERO, NSSize::new(options.size.width, options.size.height));

        let inner = BaseviewView {
            state: Rc::new(WindowState::new()),

            deferred_events: RefCell::default(),
            keyboard_state: KeyboardState::new(),
            frame_timer: None.into(),
            window_handler: None.into(),

            #[cfg(feature = "opengl")]
            gl_context: std::cell::OnceCell::new(),
        };

        let view = View::new(view_rect, inner, |view| {
            #[cfg(feature = "opengl")]
            if let Some(gl_config) = options.gl_config {
                let gl_context = crate::gl::GlContext::create(&view.view, gl_config).unwrap();
                let _ = view.gl_context.set(gl_context);
            }

            let handler = builder(&mut view.into());
            view.window_handler.replace(Some(Box::new(handler)));
        });

        view
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

    fn trigger_frame(&self) {
        let mut window = crate::Window::new(Window { inner: &self.window_inner });
        let mut window_handler = self.window_handler.borrow_mut();
        window_handler.on_frame(&mut window);
        self.send_deferred_events(window_handler.as_mut());
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

impl ViewImpl for BaseviewView {
    fn init(&self, view: &Retained<View<Self>>) {
        let timer_view = Weak::from_retained(view);
        self.frame_timer.set(TimerHandle::new(0.015, move || {
            if let Some(view) = timer_view.load() {
                view.inner().trigger_frame();
            }
        }));
    }

    fn become_first_responder(this: ViewRef<Self>) -> bool {
        let Some(window) = this.view.window() else {
            return true;
        };

        if window.isKeyWindow() {
            this.trigger_deferrable_event(Event::Window(WindowEvent::Focused));
        }

        true
    }

    fn resign_first_responder(this: ViewRef<Self>) -> bool {
        this.trigger_deferrable_event(Event::Window(WindowEvent::Unfocused));
        true
    }

    fn window_should_close(this: ViewRef<Self>) -> bool {
        this.trigger_event(Event::Window(WindowEvent::WillClose));

        //state.window_inner.close();

        false
    }

    fn view_did_change_backing_properties(this: ViewRef<Self>) {
        let ns_window = this.view.window();

        let scale_factor: f64 = ns_window.map(|w| w.backingScaleFactor()).unwrap_or(1.0);

        // SAFETY: This is our own view instance
        let state = &this.state;

        let bounds = this.view.bounds();

        let new_window_info = WindowInfo::from_logical_size(
            Size::new(bounds.size.width, bounds.size.height),
            scale_factor,
        );

        let window_info = state.window_info.get();

        // Only send the event when the window's size has actually changed to be in line with the
        // other platform implementations
        if new_window_info.physical_size() != window_info.physical_size() {
            state.window_info.set(new_window_info);
            this.trigger_event(Event::Window(WindowEvent::Resized(new_window_info)));
        }
    }

    /// `hitTest:` override that collapses hits on baseview's internal
    /// OpenGL render subview to this NSView.
    ///
    /// `src/gl/macos.rs` attaches an `NSOpenGLView` as a subview of this
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
    fn hit_test(this: ViewRef<Self>, point: NSPoint) -> Option<&NSView> {
        let superclass = this.view.class().superclass().unwrap();

        // SAFETY: Our superclass is NSView
        let super_result: Option<&NSView> =
            unsafe { msg_send![super(this.view, superclass), hitTest: point] };
        let super_result = super_result?;

        #[cfg(feature = "opengl")]
        {
            if let Some(gl_context) = this.gl_context.get() {
                if super_result == gl_context.ns_view() {
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

        this.trigger_event(Event::Mouse(MouseEvent::CursorMoved {
            position,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn scroll_wheel(this: ViewRef<Self>, event: &NSEvent) {
        let x = event.scrollingDeltaX() as f32;
        let y = event.scrollingDeltaY() as f32;

        let delta = if event.hasPreciseScrollingDeltas() {
            ScrollDelta::Pixels { x, y }
        } else {
            ScrollDelta::Lines { x, y }
        };

        this.trigger_event(Event::Mouse(MouseEvent::WheelScrolled {
            delta,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
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

        on_event(&this, event)
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

        on_event(&this, event)
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

        let event_status = this.trigger_event(Event::Mouse(event));

        match event_status {
            EventStatus::AcceptDrop(_) => true,
            _ => false,
        }
    }

    fn dragging_exited(this: ViewRef<Self>, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
        on_event(&this, MouseEvent::DragLeft);
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

        this.trigger_event(Event::Window(if window.isKeyWindow() {
            WindowEvent::Focused
        } else {
            WindowEvent::Unfocused
        }));
    }

    fn mouse_down(this: ViewRef<Self>, event: &NSEvent) {
        this.trigger_event(Event::Mouse(ButtonPressed {
            button: MouseButton::Left,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn mouse_up(this: ViewRef<Self>, event: &NSEvent) {
        this.trigger_event(Event::Mouse(ButtonReleased {
            button: MouseButton::Left,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn right_mouse_down(this: ViewRef<Self>, event: &NSEvent) {
        this.trigger_event(Event::Mouse(ButtonPressed {
            button: MouseButton::Right,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn right_mouse_up(this: ViewRef<Self>, event: &NSEvent) {
        this.trigger_event(Event::Mouse(ButtonReleased {
            button: MouseButton::Right,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn other_mouse_down(this: ViewRef<Self>, event: &NSEvent) {
        this.trigger_event(Event::Mouse(ButtonPressed {
            button: MouseButton::Middle,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn other_mouse_up(this: ViewRef<Self>, event: &NSEvent) {
        this.trigger_event(Event::Mouse(ButtonReleased {
            button: MouseButton::Middle,
            modifiers: make_modifiers(event.modifierFlags()),
        }));
    }

    fn mouse_entered(this: ViewRef<Self>) {
        this.trigger_event(Event::Mouse(MouseEvent::CursorEntered));
    }

    fn mouse_exited(this: ViewRef<Self>) {
        this.trigger_event(Event::Mouse(MouseEvent::CursorLeft));
    }

    fn key_down(this: ViewRef<Self>, event: &NSEvent) {
        if let Some(key_event) = this.keyboard_state.process_native_event(event) {
            let status = this.trigger_event(Event::Keyboard(key_event));

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
            let status = this.trigger_event(Event::Keyboard(key_event));

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
            let status = this.trigger_event(Event::Keyboard(key_event));

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

fn on_event(window_state: &BaseviewView, event: MouseEvent) -> NSDragOperation {
    let event_status = window_state.trigger_event(Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(DropEffect::Copy) => NSDragOperation::Copy,
        EventStatus::AcceptDrop(DropEffect::Move) => NSDragOperation::Move,
        EventStatus::AcceptDrop(DropEffect::Link) => NSDragOperation::Link,
        EventStatus::AcceptDrop(DropEffect::Scroll) => NSDragOperation::Generic,
        _ => NSDragOperation::None,
    }
}
