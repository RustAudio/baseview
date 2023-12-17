use std::ffi::c_void;

use cocoa::appkit::{NSEvent, NSFilenamesPboardType, NSView, NSWindow};
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSArray, NSPoint, NSRect, NSSize, NSUInteger};

use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use uuid::Uuid;

use crate::MouseEvent::{ButtonPressed, ButtonReleased};
use crate::{
    DropData, DropEffect, Event, EventStatus, MouseButton, MouseEvent, Point, ScrollDelta, Size,
    WindowEvent, WindowInfo, WindowOpenOptions,
};

use super::keyboard::{from_nsstring, make_modifiers};
use super::window::WindowState;
use super::{
    NSDragOperationCopy, NSDragOperationGeneric, NSDragOperationLink, NSDragOperationMove,
    NSDragOperationNone,
};

/// Name of the field used to store the `WindowState` pointer.
pub(super) const BASEVIEW_STATE_IVAR: &str = "baseview_state";

macro_rules! add_simple_mouse_class_method {
    ($class:ident, $sel:ident, $event:expr) => {
        #[allow(non_snake_case)]
        extern "C" fn $sel(this: &Object, _: Sel, _: id){
            let state = unsafe { WindowState::from_view(this) };

            state.trigger_event(Event::Mouse($event));
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C" fn(&Object, Sel, id),
        );
    };
}

/// Similar to [add_simple_mouse_class_method!], but this creates its own event object for the
/// press/release event and adds the active modifier keys to that event.
macro_rules! add_mouse_button_class_method {
    ($class:ident, $sel:ident, $event_ty:ident, $button:expr) => {
        #[allow(non_snake_case)]
        extern "C" fn $sel(this: &Object, _: Sel, event: id){
            let state = unsafe { WindowState::from_view(this) };

            let modifiers = unsafe { NSEvent::modifierFlags(event) };

            state.trigger_event(Event::Mouse($event_ty {
                button: $button,
                modifiers: make_modifiers(modifiers),
            }));
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C" fn(&Object, Sel, id),
        );
    };
}

macro_rules! add_simple_keyboard_class_method {
    ($class:ident, $sel:ident) => {
        #[allow(non_snake_case)]
        extern "C" fn $sel(this: &Object, _: Sel, event: id){
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

        $class.add_method(
            sel!($sel:),
            $sel as extern "C" fn(&Object, Sel, id),
        );
    };
}

pub(super) unsafe fn create_view(window_options: &WindowOpenOptions) -> id {
    let class = create_view_class();

    let view: id = msg_send![class, alloc];

    let size = window_options.size;

    view.initWithFrame_(NSRect::new(NSPoint::new(0., 0.), NSSize::new(size.width, size.height)));

    let _: id = msg_send![
        view,
        registerForDraggedTypes: NSArray::arrayWithObjects(nil, &[NSFilenamesPboardType])
    ];

    view
}

unsafe fn create_view_class() -> &'static Class {
    // Use unique class names so that there are no conflicts between different
    // instances. The class is deleted when the view is released. Previously,
    // the class was stored in a OnceCell after creation. This way, we didn't
    // have to recreate it each time a view was opened, but now we don't leave
    // any class definitions lying around when the plugin is closed.
    let class_name = format!("BaseviewNSView_{}", Uuid::new_v4().to_simple());
    let mut class = ClassDecl::new(&class_name, class!(NSView)).unwrap();

    class.add_method(
        sel!(acceptsFirstResponder),
        property_yes as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(sel!(isFlipped), property_yes as extern "C" fn(&Object, Sel) -> BOOL);
    class.add_method(
        sel!(preservesContentInLiveResize),
        property_no as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, id) -> BOOL,
    );

    class.add_method(
        sel!(windowShouldClose:),
        window_should_close as extern "C" fn(&Object, Sel, id) -> BOOL,
    );
    class.add_method(sel!(dealloc), dealloc as extern "C" fn(&mut Object, Sel));
    class.add_method(
        sel!(viewWillMoveToWindow:),
        view_will_move_to_window as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(updateTrackingAreas:),
        update_tracking_areas as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(&Object, Sel, id));
    class.add_method(sel!(mouseDragged:), mouse_moved as extern "C" fn(&Object, Sel, id));
    class.add_method(sel!(rightMouseDragged:), mouse_moved as extern "C" fn(&Object, Sel, id));
    class.add_method(sel!(otherMouseDragged:), mouse_moved as extern "C" fn(&Object, Sel, id));

    class.add_method(sel!(scrollWheel:), scroll_wheel as extern "C" fn(&Object, Sel, id));

    class.add_method(
        sel!(viewDidChangeBackingProperties:),
        view_did_change_backing_properties as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(draggingEntered:),
        dragging_entered as extern "C" fn(&Object, Sel, id) -> NSUInteger,
    );
    class.add_method(
        sel!(prepareForDragOperation:),
        prepare_for_drag_operation as extern "C" fn(&Object, Sel, id) -> BOOL,
    );
    class.add_method(
        sel!(performDragOperation:),
        perform_drag_operation as extern "C" fn(&Object, Sel, id) -> BOOL,
    );
    class.add_method(
        sel!(draggingUpdated:),
        dragging_updated as extern "C" fn(&Object, Sel, id) -> NSUInteger,
    );
    class.add_method(sel!(draggingExited:), dragging_exited as extern "C" fn(&Object, Sel, id));

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

    class.add_ivar::<*mut c_void>(BASEVIEW_STATE_IVAR);

    class.register()
}

extern "C" fn property_yes(_this: &Object, _sel: Sel) -> BOOL {
    YES
}

extern "C" fn property_no(_this: &Object, _sel: Sel) -> BOOL {
    NO
}

extern "C" fn accepts_first_mouse(_this: &Object, _sel: Sel, _event: id) -> BOOL {
    YES
}

extern "C" fn window_should_close(this: &Object, _: Sel, _sender: id) -> BOOL {
    let state = unsafe { WindowState::from_view(this) };

    state.trigger_event(Event::Window(WindowEvent::WillClose));

    state.window_inner.close();

    NO
}

extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
    unsafe {
        let class = msg_send![this, class];

        let superclass = msg_send![this, superclass];
        let () = msg_send![super(this, superclass), dealloc];

        // Delete class
        ::objc::runtime::objc_disposeClassPair(class);
    }
}

extern "C" fn view_did_change_backing_properties(this: &Object, _: Sel, _: id) {
    unsafe {
        let ns_window: *mut Object = msg_send![this, window];

        let scale_factor: f64 =
            if ns_window.is_null() { 1.0 } else { NSWindow::backingScaleFactor(ns_window) };

        let state = WindowState::from_view(this);

        let bounds: NSRect = msg_send![this, bounds];

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
}

/// Init/reinit tracking area
///
/// Info:
/// https://developer.apple.com/documentation/appkit/nstrackingarea
/// https://developer.apple.com/documentation/appkit/nstrackingarea/options
/// https://developer.apple.com/documentation/appkit/nstrackingareaoptions
unsafe fn reinit_tracking_area(this: &Object, tracking_area: *mut Object) {
    let options: usize = {
        let mouse_entered_and_exited = 0x01;
        let tracking_mouse_moved = 0x02;
        let tracking_cursor_update = 0x04;
        let tracking_active_in_active_app = 0x40;
        let tracking_in_visible_rect = 0x200;
        let tracking_enabled_during_mouse_drag = 0x400;

        mouse_entered_and_exited
            | tracking_mouse_moved
            | tracking_cursor_update
            | tracking_active_in_active_app
            | tracking_in_visible_rect
            | tracking_enabled_during_mouse_drag
    };

    let bounds: NSRect = msg_send![this, bounds];

    *tracking_area = msg_send![tracking_area,
        initWithRect:bounds
        options:options
        owner:this
        userInfo:nil
    ];
}

extern "C" fn view_will_move_to_window(this: &Object, _self: Sel, new_window: id) {
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area_count = NSArray::count(tracking_areas);

        let _: () = msg_send![class!(NSEvent), setMouseCoalescingEnabled: NO];

        if new_window == nil {
            if tracking_area_count != 0 {
                let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);

                let _: () = msg_send![this, removeTrackingArea: tracking_area];
                let _: () = msg_send![tracking_area, release];
            }
        } else {
            if tracking_area_count == 0 {
                let class = Class::get("NSTrackingArea").unwrap();

                let tracking_area: *mut Object = msg_send![class, alloc];

                reinit_tracking_area(this, tracking_area);

                let _: () = msg_send![this, addTrackingArea: tracking_area];
            }

            let _: () = msg_send![new_window, setAcceptsMouseMovedEvents: YES];
            let _: () = msg_send![new_window, makeFirstResponder: this];
        }
    }

    unsafe {
        let superclass = msg_send![this, superclass];

        let () = msg_send![super(this, superclass), viewWillMoveToWindow: new_window];
    }
}

extern "C" fn update_tracking_areas(this: &Object, _self: Sel, _: id) {
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);

        reinit_tracking_area(this, tracking_area);
    }
}

extern "C" fn mouse_moved(this: &Object, _sel: Sel, event: id) {
    let state = unsafe { WindowState::from_view(this) };

    let point: NSPoint = unsafe {
        let point = NSEvent::locationInWindow(event);

        msg_send![this, convertPoint:point fromView:nil]
    };
    let modifiers = unsafe { NSEvent::modifierFlags(event) };

    let position = Point { x: point.x, y: point.y };

    state.trigger_event(Event::Mouse(MouseEvent::CursorMoved {
        position,
        modifiers: make_modifiers(modifiers),
    }));
}

extern "C" fn scroll_wheel(this: &Object, _: Sel, event: id) {
    let state = unsafe { WindowState::from_view(this) };

    let delta = unsafe {
        let x = NSEvent::scrollingDeltaX(event) as f32;
        let y = NSEvent::scrollingDeltaY(event) as f32;

        if NSEvent::hasPreciseScrollingDeltas(event) != NO {
            ScrollDelta::Pixels { x, y }
        } else {
            ScrollDelta::Lines { x, y }
        }
    };

    let modifiers = unsafe { NSEvent::modifierFlags(event) };

    state.trigger_event(Event::Mouse(MouseEvent::WheelScrolled {
        delta,
        modifiers: make_modifiers(modifiers),
    }));
}

fn get_drag_position(sender: id) -> Point {
    let point: NSPoint = unsafe { msg_send![sender, draggingLocation] };
    Point::new(point.x, point.y)
}

fn get_drop_data(sender: id) -> DropData {
    if sender == nil {
        return DropData::None;
    }

    unsafe {
        let pasteboard: id = msg_send![sender, draggingPasteboard];
        let file_list: id = msg_send![pasteboard, propertyListForType: NSFilenamesPboardType];

        if file_list == nil {
            return DropData::None;
        }

        let mut files = vec![];
        for i in 0..NSArray::count(file_list) {
            let data = NSArray::objectAtIndex(file_list, i);
            files.push(from_nsstring(data).into());
        }

        DropData::Files(files)
    }
}

fn on_event(window_state: &WindowState, event: MouseEvent) -> NSUInteger {
    let event_status = window_state.trigger_event(Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(DropEffect::Copy) => NSDragOperationCopy,
        EventStatus::AcceptDrop(DropEffect::Move) => NSDragOperationMove,
        EventStatus::AcceptDrop(DropEffect::Link) => NSDragOperationLink,
        EventStatus::AcceptDrop(DropEffect::Scroll) => NSDragOperationGeneric,
        _ => NSDragOperationNone,
    }
}

extern "C" fn dragging_entered(this: &Object, _sel: Sel, sender: id) -> NSUInteger {
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

extern "C" fn dragging_updated(this: &Object, _sel: Sel, sender: id) -> NSUInteger {
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

extern "C" fn prepare_for_drag_operation(_this: &Object, _sel: Sel, _sender: id) -> BOOL {
    // Always accept drag operation if we get this far
    // This function won't be called unless dragging_entered/updated
    // has returned an acceptable operation
    YES
}

extern "C" fn perform_drag_operation(this: &Object, _sel: Sel, sender: id) -> BOOL {
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
        EventStatus::AcceptDrop(_) => YES,
        _ => NO,
    }
}

extern "C" fn dragging_exited(this: &Object, _sel: Sel, _sender: id) {
    let state = unsafe { WindowState::from_view(this) };

    on_event(&state, MouseEvent::DragLeft);
}
