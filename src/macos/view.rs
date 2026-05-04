#![allow(deprecated)] // Allow use of NSFilenamesPboardType for now

use objc2::__framework_prelude::Retained;
use objc2::ffi::objc_disposeClassPair;
use objc2::rc::Allocated;
use objc2::runtime::{
    AnyClass, AnyObject, Bool, ClassBuilder, NSObjectProtocol, ProtocolObject, Sel,
};
use objc2::{msg_send, sel, AllocAnyThread, ClassType};
use objc2_app_kit::{
    NSDragOperation, NSDraggingInfo, NSEvent, NSFilenamesPboardType, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindow, NSWindowDidBecomeKeyNotification,
    NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSPoint, NSRect, NSSize, NSString,
};
use std::ffi::{c_void, CStr, CString};
use uuid::Uuid;

use super::keyboard::make_modifiers;
use super::window::WindowState;
use crate::MouseEvent::{ButtonPressed, ButtonReleased};
use crate::{
    DropData, DropEffect, Event, EventStatus, MouseButton, MouseEvent, Point, ScrollDelta, Size,
    WindowEvent, WindowInfo, WindowOpenOptions,
};

/// Name of the field used to store the `WindowState` pointer.
pub(super) const BASEVIEW_STATE_IVAR: &CStr = c"baseview_state";

macro_rules! add_simple_mouse_class_method {
    ($class:ident, $sel:ident, $event:expr) => {
        #[allow(non_snake_case)]
        extern "C-unwind" fn $sel(this: &NSView, _: Sel, _: &AnyObject){
            let state = unsafe { WindowState::from_view(this) };

            state.trigger_event(Event::Mouse($event));
        }

        $class.add_method(sel!($sel:), $sel as extern "C-unwind" fn(_, _, _) -> _,);
    };
}

/// Similar to [add_simple_mouse_class_method!], but this creates its own event object for the
/// press/release event and adds the active modifier keys to that event.
macro_rules! add_mouse_button_class_method {
    ($class:ident, $sel:ident, $event_ty:ident, $button:expr) => {
        #[allow(non_snake_case)]
        extern "C-unwind" fn $sel(this: &NSView, _: Sel, event: &NSEvent){
            let state = unsafe { WindowState::from_view(this) };

            state.trigger_event(Event::Mouse($event_ty {
                button: $button,
                modifiers: make_modifiers(event.modifierFlags()),
            }));
        }

        $class.add_method(sel!($sel:),$sel as extern "C-unwind" fn(_, _, _) -> _);
    };
}

macro_rules! add_simple_keyboard_class_method {
    ($class:ident, $sel:ident) => {
        #[allow(non_snake_case)]
        extern "C-unwind" fn $sel(this: &NSView, _: Sel, event: &NSEvent){
            let state = unsafe { WindowState::from_view(this) };

            if let Some(key_event) = state.process_native_key_event(event){
                let status = state.trigger_event(Event::Keyboard(key_event));

                if let EventStatus::Ignored = status {
                    unsafe {
                        let superclass = msg_send![this, superclass];

                        let () = msg_send![super(this, superclass), $sel:event];
                    }
                }
            }
        }

        $class.add_method(sel!($sel:),$sel as extern "C-unwind" fn(_, _, _) -> _);
    };
}

pub(super) fn create_view(window_options: &WindowOpenOptions) -> Retained<NSView> {
    let class = create_view_class();
    let view: Allocated<NSView> = unsafe { msg_send![class, alloc] };

    let size = window_options.size;
    let view = NSView::initWithFrame(
        view,
        NSRect::new(NSPoint::ZERO, NSSize::new(size.width, size.height)),
    );

    let notification_center = NSNotificationCenter::defaultCenter();

    // SAFETY: TODO
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
    }

    // SAFETY: TODO
    let ns_filenames_pboard_type = unsafe { NSFilenamesPboardType };
    view.registerForDraggedTypes(&NSArray::from_slice(&[ns_filenames_pboard_type]));

    view
}

fn create_view_class() -> &'static AnyClass {
    // Use unique class names so that there are no conflicts between different
    // instances. The class is deleted when the view is released. Previously,
    // the class was stored in a OnceCell after creation. This way, we didn't
    // have to recreate it each time a view was opened, but now we don't leave
    // any class definitions lying around when the plugin is closed.
    let class_name = CString::new(format!("BaseviewNSView_{}", Uuid::new_v4().simple()))
        // PANIC: This cannot have any NULL bytes
        .unwrap();

    let mut class = ClassBuilder::new(&class_name, NSView::class()).unwrap();

    // SAFETY: All of these function signatures are correct
    unsafe {
        class.add_method(
            sel!(acceptsFirstResponder),
            property_yes as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(
            sel!(becomeFirstResponder),
            become_first_responder as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(
            sel!(resignFirstResponder),
            resign_first_responder as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(sel!(isFlipped), property_yes as extern "C-unwind" fn(_, _) -> _);
        class.add_method(
            sel!(preservesContentInLiveResize),
            property_no as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(
            sel!(acceptsFirstMouse:),
            accepts_first_mouse as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(
            sel!(windowShouldClose:),
            window_should_close as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(sel!(dealloc), dealloc as extern "C-unwind" fn(_, _));
        class.add_method(
            sel!(viewWillMoveToWindow:),
            view_will_move_to_window as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(sel!(hitTest:), hit_test as extern "C-unwind" fn(_, _, _) -> _);
        class.add_method(
            sel!(updateTrackingAreas:),
            update_tracking_areas as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(sel!(mouseMoved:), mouse_moved as extern "C-unwind" fn(_, _, _) -> _);
        class.add_method(sel!(mouseDragged:), mouse_moved as extern "C-unwind" fn(_, _, _) -> _);
        class.add_method(
            sel!(rightMouseDragged:),
            mouse_moved as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(otherMouseDragged:),
            mouse_moved as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(sel!(scrollWheel:), scroll_wheel as extern "C-unwind" fn(_, _, _) -> _);

        class.add_method(
            sel!(viewDidChangeBackingProperties:),
            view_did_change_backing_properties as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(
            sel!(draggingEntered:),
            dragging_entered as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(prepareForDragOperation:),
            prepare_for_drag_operation as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(performDragOperation:),
            perform_drag_operation as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(draggingUpdated:),
            dragging_updated as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(draggingExited:),
            dragging_exited as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(handleNotification:),
            handle_notification as extern "C-unwind" fn(_, _, _) -> _,
        );

        add_mouse_button_class_method!(class, mouseDown, ButtonPressed, MouseButton::Left);
        add_mouse_button_class_method!(class, mouseUp, ButtonReleased, MouseButton::Left);
        add_mouse_button_class_method!(class, rightMouseDown, ButtonPressed, MouseButton::Right);
        add_mouse_button_class_method!(class, rightMouseUp, ButtonReleased, MouseButton::Right);
        add_mouse_button_class_method!(class, otherMouseDown, ButtonPressed, MouseButton::Middle);
        add_mouse_button_class_method!(class, otherMouseUp, ButtonReleased, MouseButton::Middle);
        add_simple_mouse_class_method!(class, mouseEntered, MouseEvent::CursorEntered);
        add_simple_mouse_class_method!(class, mouseExited, MouseEvent::CursorLeft);

        add_simple_keyboard_class_method!(class, keyDown);
        add_simple_keyboard_class_method!(class, keyUp);
        add_simple_keyboard_class_method!(class, flagsChanged);
    }

    class.add_ivar::<*mut c_void>(BASEVIEW_STATE_IVAR);

    class.register()
}

extern "C-unwind" fn property_yes(_this: &NSView, _sel: Sel) -> Bool {
    Bool::YES
}

extern "C-unwind" fn property_no(_this: &NSView, _sel: Sel) -> Bool {
    Bool::NO
}

extern "C-unwind" fn accepts_first_mouse(_this: &NSView, _sel: Sel, _event: &NSEvent) -> Bool {
    Bool::YES
}

extern "C-unwind" fn become_first_responder(this: &NSView, _sel: Sel) -> Bool {
    let Some(window) = this.window() else {
        return Bool::YES;
    };

    if window.isKeyWindow() {
        let state = unsafe { WindowState::from_view(this) };
        state.trigger_deferrable_event(Event::Window(WindowEvent::Focused));
    }

    Bool::YES
}

extern "C-unwind" fn resign_first_responder(this: &NSView, _sel: Sel) -> Bool {
    let state = unsafe { WindowState::from_view(this) };
    state.trigger_deferrable_event(Event::Window(WindowEvent::Unfocused));
    Bool::YES
}

extern "C-unwind" fn window_should_close(this: &NSView, _: Sel, _sender: &AnyObject) -> Bool {
    let state = unsafe { WindowState::from_view(this) };

    state.trigger_event(Event::Window(WindowEvent::WillClose));

    state.window_inner.close();

    Bool::NO
}

extern "C-unwind" fn dealloc(this: &mut AnyObject, _sel: Sel) {
    let class = this.class();

    if let Some(superclass) = class.superclass() {
        let () = unsafe { msg_send![super(this, superclass), dealloc] };
    }

    // Delete class
    // SAFETY: TODO: nope, this is NOT sound, as this invalidates any &AnyClass
    unsafe { objc_disposeClassPair(class as *const _ as *mut _) }
}

extern "C-unwind" fn view_did_change_backing_properties(this: &NSView, _: Sel, _: &AnyObject) {
    let ns_window = this.window();

    let scale_factor: f64 = ns_window.map(|w| w.backingScaleFactor()).unwrap_or(1.0);

    // SAFETY: TODO
    let state = unsafe { WindowState::from_view(this) };

    let bounds = this.bounds();

    let new_window_info = WindowInfo::from_logical_size(
        Size::new(bounds.size.width, bounds.size.height),
        scale_factor,
    );

    let window_info = state.window_info.get();

    // Only send the event when the window's size has actually changed to be in line with the
    // other platform implementations
    if new_window_info.physical_size() != window_info.physical_size() {
        state.window_info.set(new_window_info);
        state.trigger_event(Event::Window(WindowEvent::Resized(new_window_info)));
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
extern "C-unwind" fn hit_test(this: &NSView, _sel: Sel, point: NSPoint) -> Option<&NSView> {
    // SAFETY: TODO
    let super_result: Option<&NSView> = unsafe { msg_send![super(this), hitTest: point] };
    let super_result = super_result?;

    #[cfg(feature = "opengl")]
    {
        let state = unsafe { WindowState::from_view(this) };
        if let Some(gl_context) = state.window_inner.gl_context.as_ref() {
            if super_result == gl_context.ns_view() {
                return Some(this);
            }
        }
    }

    Some(super_result)
}

extern "C-unwind" fn view_will_move_to_window(
    this: &NSView, _self: Sel, new_window: Option<&NSWindow>,
) {
    let tracking_areas = this.trackingAreas();

    match new_window {
        None => {
            if tracking_areas.count() > 0 {
                let tracking_area = tracking_areas.objectAtIndex(0);
                this.removeTrackingArea(&tracking_area);
            }
        }
        Some(new_window) => {
            if tracking_areas.is_empty() {
                let tracking_area = new_tracking_area(this);
                this.addTrackingArea(&tracking_area);
            }

            new_window.setAcceptsMouseMovedEvents(true);
            new_window.makeFirstResponder(Some(this));
        }
    }

    unsafe {
        let superclass = msg_send![this, superclass];

        let () = msg_send![super(this, superclass), viewWillMoveToWindow: new_window];
    }
}

extern "C-unwind" fn update_tracking_areas(this: &NSView, _self: Sel, _: &AnyObject) {
    let tracking_areas = this.trackingAreas();
    if tracking_areas.count() > 0 {
        let tracking_area = tracking_areas.objectAtIndex(0);
        this.removeTrackingArea(&tracking_area);
    }

    let tracking_area = new_tracking_area(this);

    this.addTrackingArea(&tracking_area);
}

extern "C-unwind" fn mouse_moved(this: &NSView, _sel: Sel, event: &NSEvent) {
    let state = unsafe { WindowState::from_view(this) };
    let point = this.convertPoint_fromView(event.locationInWindow(), None);

    let position = Point { x: point.x, y: point.y };

    state.trigger_event(Event::Mouse(MouseEvent::CursorMoved {
        position,
        modifiers: make_modifiers(event.modifierFlags()),
    }));
}

extern "C-unwind" fn scroll_wheel(this: &NSView, _: Sel, event: &NSEvent) {
    let state = unsafe { WindowState::from_view(this) };

    let x = event.scrollingDeltaX() as f32;
    let y = event.scrollingDeltaY() as f32;

    let delta = if event.hasPreciseScrollingDeltas() {
        ScrollDelta::Pixels { x, y }
    } else {
        ScrollDelta::Lines { x, y }
    };

    state.trigger_event(Event::Mouse(MouseEvent::WheelScrolled {
        delta,
        modifiers: make_modifiers(event.modifierFlags()),
    }));
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
        .filter_map(|i| i.downcast::<NSString>().ok())
        .map(|s| s.to_string().into())
        .collect();

    DropData::Files(files)
}

fn on_event(window_state: &WindowState, event: MouseEvent) -> NSDragOperation {
    let event_status = window_state.trigger_event(Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(DropEffect::Copy) => NSDragOperation::Copy,
        EventStatus::AcceptDrop(DropEffect::Move) => NSDragOperation::Move,
        EventStatus::AcceptDrop(DropEffect::Link) => NSDragOperation::Link,
        EventStatus::AcceptDrop(DropEffect::Scroll) => NSDragOperation::Generic,
        _ => NSDragOperation::None,
    }
}

extern "C-unwind" fn dragging_entered(
    this: &NSView, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> NSDragOperation {
    let state = unsafe { WindowState::from_view(this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender);

    let event = MouseEvent::DragEntered {
        position: get_drag_position(sender),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    on_event(&state, event)
}

extern "C-unwind" fn dragging_updated(
    this: &NSView, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> NSDragOperation {
    let state = unsafe { WindowState::from_view(this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender);

    let event = MouseEvent::DragMoved {
        position: get_drag_position(sender),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    on_event(&state, event)
}

extern "C-unwind" fn prepare_for_drag_operation(
    _this: &NSView, _sel: Sel, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> Bool {
    // Always accept drag operation if we get this far
    // This function won't be called unless dragging_entered/updated
    // has returned an acceptable operation
    Bool::YES
}

extern "C-unwind" fn perform_drag_operation(
    this: &NSView, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> Bool {
    let state = unsafe { WindowState::from_view(this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender);

    let event = MouseEvent::DragDropped {
        position: get_drag_position(sender),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    let event_status = state.trigger_event(Event::Mouse(event));

    match event_status {
        EventStatus::AcceptDrop(_) => Bool::YES,
        _ => Bool::NO,
    }
}

extern "C-unwind" fn dragging_exited(
    this: &NSView, _sel: Sel, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) {
    let state = unsafe { WindowState::from_view(this) };

    on_event(&state, MouseEvent::DragLeft);
}

extern "C-unwind" fn handle_notification(this: &NSView, _cmd: Sel, notification: &NSNotification) {
    let state = unsafe { WindowState::from_view(this) };

    let Some(window) = this.window() else { return };
    // The subject of the notification, in this case an NSWindow object.
    let Some(notification_object) = notification.object().and_then(|o| o.downcast().ok()) else {
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
    if !this.isEqual(Some(&first_responder)) {
        return;
    }

    state.trigger_event(Event::Window(if window.isKeyWindow() {
        WindowEvent::Focused
    } else {
        WindowEvent::Unfocused
    }));
}
