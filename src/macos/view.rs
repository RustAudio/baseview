use std::ffi::{c_void, CStr, CString};

use cocoa::appkit::{NSEvent, NSFilenamesPboardType, NSView, NSWindow};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSArray, NSPoint, NSRect, NSSize, NSUInteger};

use objc2::{
    class, msg_send,
    runtime::{AnyClass, AnyObject, Bool as ObjcBool, ClassBuilder, Sel},
    sel, Encode, Encoding,
};
use uuid::Uuid;

/// `CGPoint`/`CGSize`/`CGRect` clones carrying an `objc2::Encode` impl. Layout-identical to
/// cocoa's `NSPoint`/`NSSize`/`NSRect`, so `From` is a field-wise copy. We need these because
/// cocoa's types are external and can't implement objc2's `Encode` trait.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CgPoint {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CgSize {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CgRect {
    origin: CgPoint,
    size: CgSize,
}

unsafe impl Encode for CgPoint {
    const ENCODING: Encoding =
        Encoding::Struct("CGPoint", &[<f64 as Encode>::ENCODING, <f64 as Encode>::ENCODING]);
}
unsafe impl Encode for CgSize {
    const ENCODING: Encoding =
        Encoding::Struct("CGSize", &[<f64 as Encode>::ENCODING, <f64 as Encode>::ENCODING]);
}
unsafe impl Encode for CgRect {
    const ENCODING: Encoding =
        Encoding::Struct("CGRect", &[<CgPoint as Encode>::ENCODING, <CgSize as Encode>::ENCODING]);
}

impl From<CgRect> for NSRect {
    fn from(r: CgRect) -> Self {
        NSRect::new(NSPoint::new(r.origin.x, r.origin.y), NSSize::new(r.size.width, r.size.height))
    }
}
impl From<NSPoint> for CgPoint {
    fn from(p: NSPoint) -> Self {
        CgPoint { x: p.x, y: p.y }
    }
}
impl From<CgPoint> for NSPoint {
    fn from(p: CgPoint) -> Self {
        NSPoint::new(p.x, p.y)
    }
}

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

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    static NSWindowDidBecomeKeyNotification: id;
    static NSWindowDidResignKeyNotification: id;
}

macro_rules! add_simple_mouse_class_method {
    ($class:ident, $sel:ident, $event:expr) => {
        #[allow(non_snake_case)]
        extern "C-unwind" fn $sel(this: *const AnyObject, _: Sel, _: *mut AnyObject){
            let state = unsafe { WindowState::from_view(&*this) };

            state.trigger_event(Event::Mouse($event));
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
        );
    };
}

/// Similar to [add_simple_mouse_class_method!], but this creates its own event object for the
/// press/release event and adds the active modifier keys to that event.
macro_rules! add_mouse_button_class_method {
    ($class:ident, $sel:ident, $event_ty:ident, $button:expr) => {
        #[allow(non_snake_case)]
        extern "C-unwind" fn $sel(this: *const AnyObject, _: Sel, event: *mut AnyObject){
            let state = unsafe { WindowState::from_view(&*this) };

            let modifiers = unsafe { NSEvent::modifierFlags(event as id) };

            state.trigger_event(Event::Mouse($event_ty {
                button: $button,
                modifiers: make_modifiers(modifiers),
            }));
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
        );
    };
}

macro_rules! add_simple_keyboard_class_method {
    ($class:ident, $sel:ident) => {
        #[allow(non_snake_case)]
        extern "C-unwind" fn $sel(this: *const AnyObject, _: Sel, event: *mut AnyObject){
            let state = unsafe { WindowState::from_view(&*this) };

            if let Some(key_event) = state.process_native_key_event(event){
                let status = state.trigger_event(Event::Keyboard(key_event));

                if let EventStatus::Ignored = status {
                    unsafe {
                        let superclass: &AnyClass = msg_send![this, superclass];

                        let () = msg_send![super(&*this, superclass), $sel:event];
                    }
                }
            }
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
        );
    };
}

unsafe fn register_notification(observer: id, notification_name: id, object: id) {
    let notification_center: *mut AnyObject =
        msg_send![class!(NSNotificationCenter), defaultCenter];

    let _: () = msg_send![
        notification_center,
        addObserver: observer as *mut AnyObject,
        selector: sel!(handleNotification:),
        name: notification_name as *mut AnyObject,
        object: object as *mut AnyObject,
    ];
}

pub(super) unsafe fn create_view(window_options: &WindowOpenOptions) -> id {
    let class = create_view_class();

    let view_any: *mut AnyObject = msg_send![class, alloc];
    let view: id = view_any as id;

    let size = window_options.size;

    view.initWithFrame_(NSRect::new(NSPoint::new(0., 0.), NSSize::new(size.width, size.height)));

    register_notification(view, NSWindowDidBecomeKeyNotification, nil);
    register_notification(view, NSWindowDidResignKeyNotification, nil);

    let drag_types = NSArray::arrayWithObjects(nil, &[NSFilenamesPboardType]) as *mut AnyObject;
    let _: () = msg_send![view as *mut AnyObject, registerForDraggedTypes: drag_types];

    view
}

unsafe fn create_view_class() -> &'static AnyClass {
    // Use unique class names so that there are no conflicts between different
    // instances. The class is deleted when the view is released. Previously,
    // the class was stored in a OnceCell after creation. This way, we didn't
    // have to recreate it each time a view was opened, but now we don't leave
    // any class definitions lying around when the plugin is closed.
    let class_name =
        CString::new(format!("BaseviewNSView_{}", Uuid::new_v4().to_simple())).unwrap();
    let mut class = ClassBuilder::new(&class_name, class!(NSView)).unwrap();

    class.add_method(
        sel!(acceptsFirstResponder),
        property_yes as extern "C-unwind" fn(*const AnyObject, Sel) -> ObjcBool,
    );
    class.add_method(
        sel!(becomeFirstResponder),
        become_first_responder as extern "C-unwind" fn(*const AnyObject, Sel) -> ObjcBool,
    );
    class.add_method(
        sel!(resignFirstResponder),
        resign_first_responder as extern "C-unwind" fn(*const AnyObject, Sel) -> ObjcBool,
    );
    class.add_method(
        sel!(isFlipped),
        property_yes as extern "C-unwind" fn(*const AnyObject, Sel) -> ObjcBool,
    );
    class.add_method(
        sel!(preservesContentInLiveResize),
        property_no as extern "C-unwind" fn(*const AnyObject, Sel) -> ObjcBool,
    );
    class.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject) -> ObjcBool,
    );

    class.add_method(
        sel!(windowShouldClose:),
        window_should_close
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject) -> ObjcBool,
    );
    class.add_method(sel!(dealloc), dealloc as extern "C-unwind" fn(*mut AnyObject, Sel));
    class.add_method(
        sel!(viewWillMoveToWindow:),
        view_will_move_to_window as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );
    class.add_method(
        sel!(updateTrackingAreas:),
        update_tracking_areas as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );

    class.add_method(
        sel!(mouseMoved:),
        mouse_moved as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );
    class.add_method(
        sel!(mouseDragged:),
        mouse_moved as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );
    class.add_method(
        sel!(rightMouseDragged:),
        mouse_moved as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );
    class.add_method(
        sel!(otherMouseDragged:),
        mouse_moved as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );

    class.add_method(
        sel!(scrollWheel:),
        scroll_wheel as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );

    class.add_method(
        sel!(viewDidChangeBackingProperties:),
        view_did_change_backing_properties
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );

    class.add_method(
        sel!(draggingEntered:),
        dragging_entered
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject) -> NSUInteger,
    );
    class.add_method(
        sel!(prepareForDragOperation:),
        prepare_for_drag_operation
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject) -> ObjcBool,
    );
    class.add_method(
        sel!(performDragOperation:),
        perform_drag_operation
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject) -> ObjcBool,
    );
    class.add_method(
        sel!(draggingUpdated:),
        dragging_updated
            as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject) -> NSUInteger,
    );
    class.add_method(
        sel!(draggingExited:),
        dragging_exited as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
    );
    class.add_method(
        sel!(handleNotification:),
        handle_notification as extern "C-unwind" fn(*const AnyObject, Sel, *mut AnyObject),
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

    let ivar_name = CString::new(BASEVIEW_STATE_IVAR).unwrap();
    class.add_ivar::<*mut c_void>(&ivar_name);

    class.register()
}

extern "C-unwind" fn property_yes(_this: *const AnyObject, _sel: Sel) -> ObjcBool {
    ObjcBool::YES
}

extern "C-unwind" fn property_no(_this: *const AnyObject, _sel: Sel) -> ObjcBool {
    ObjcBool::NO
}

extern "C-unwind" fn accepts_first_mouse(
    _this: *const AnyObject, _sel: Sel, _event: *mut AnyObject,
) -> ObjcBool {
    ObjcBool::YES
}

extern "C-unwind" fn become_first_responder(this: *const AnyObject, _sel: Sel) -> ObjcBool {
    let state = unsafe { WindowState::from_view(&*this) };
    let is_key_window = unsafe {
        let window: *mut AnyObject = msg_send![this, window];
        if !window.is_null() {
            let is_key_window: ObjcBool = msg_send![window, isKeyWindow];
            is_key_window.as_bool()
        } else {
            false
        }
    };
    if is_key_window {
        state.trigger_deferrable_event(Event::Window(WindowEvent::Focused));
    }
    ObjcBool::YES
}

extern "C-unwind" fn resign_first_responder(this: *const AnyObject, _sel: Sel) -> ObjcBool {
    let state = unsafe { WindowState::from_view(&*this) };
    state.trigger_deferrable_event(Event::Window(WindowEvent::Unfocused));
    ObjcBool::YES
}

extern "C-unwind" fn window_should_close(
    this: *const AnyObject, _: Sel, _sender: *mut AnyObject,
) -> ObjcBool {
    let state = unsafe { WindowState::from_view(&*this) };

    state.trigger_event(Event::Window(WindowEvent::WillClose));

    state.window_inner.close();

    ObjcBool::NO
}

extern "C-unwind" fn dealloc(this: *mut AnyObject, _sel: Sel) {
    unsafe {
        let class: *const AnyClass = msg_send![this, class];

        let superclass: &AnyClass = msg_send![this, superclass];
        let () = msg_send![super(&mut *this, superclass), dealloc];

        // Delete class
        objc2::ffi::objc_disposeClassPair(class as *mut _);
    }
}

extern "C-unwind" fn view_did_change_backing_properties(
    this: *const AnyObject, _: Sel, _: *mut AnyObject,
) {
    unsafe {
        let ns_window: *mut AnyObject = msg_send![this, window];

        let scale_factor: f64 =
            if ns_window.is_null() { 1.0 } else { NSWindow::backingScaleFactor(ns_window as id) };

        let state = WindowState::from_view(&*this);

        let bounds_raw: CgRect = msg_send![this, bounds];
        let bounds: NSRect = bounds_raw.into();

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
unsafe fn reinit_tracking_area(this: *const AnyObject, tracking_area: *mut AnyObject) {
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

    let bounds_raw: CgRect = msg_send![this, bounds];

    let _: *mut AnyObject = msg_send![tracking_area,
        initWithRect: bounds_raw,
        options: options,
        owner: this,
        userInfo: std::ptr::null_mut::<AnyObject>(),
    ];
}

extern "C-unwind" fn view_will_move_to_window(
    this: *const AnyObject, _self: Sel, new_window: *mut AnyObject,
) {
    unsafe {
        let tracking_areas: *mut AnyObject = msg_send![this, trackingAreas];
        let tracking_area_count = NSArray::count(tracking_areas as id);

        if new_window.is_null() {
            if tracking_area_count != 0 {
                let tracking_area = NSArray::objectAtIndex(tracking_areas as id, 0);

                let _: () = msg_send![this, removeTrackingArea: tracking_area as *mut AnyObject];
                let _: () = msg_send![tracking_area as *mut AnyObject, release];
            }
        } else {
            if tracking_area_count == 0 {
                let class =
                    AnyClass::get(CStr::from_bytes_with_nul(b"NSTrackingArea\0").unwrap()).unwrap();

                let tracking_area: *mut AnyObject = msg_send![class, alloc];

                reinit_tracking_area(this, tracking_area);

                let _: () = msg_send![this, addTrackingArea: tracking_area];
            }

            let _: () = msg_send![new_window, setAcceptsMouseMovedEvents: ObjcBool::YES];
            let _: ObjcBool = msg_send![new_window, makeFirstResponder: this];
        }
    }

    unsafe {
        let superclass: &AnyClass = msg_send![this, superclass];

        let () = msg_send![super(&*this, superclass), viewWillMoveToWindow: new_window];
    }
}

extern "C-unwind" fn update_tracking_areas(this: *const AnyObject, _self: Sel, _: *mut AnyObject) {
    unsafe {
        let tracking_areas: *mut AnyObject = msg_send![this, trackingAreas];
        // Guard against `objectAtIndex:` raising NSRangeException — the
        // companion `view_will_move_to_window` site already does this; mirror
        // it here so an unwind out of this `extern "C-unwind"` callback can't
        // happen if AppKit ever invokes `updateTrackingAreas:` before the
        // first tracking area has been installed.
        if NSArray::count(tracking_areas as id) == 0 {
            return;
        }
        let tracking_area = NSArray::objectAtIndex(tracking_areas as id, 0);

        reinit_tracking_area(this, tracking_area as *mut AnyObject);
    }
}

extern "C-unwind" fn mouse_moved(this: *const AnyObject, _sel: Sel, event: *mut AnyObject) {
    let state = unsafe { WindowState::from_view(&*this) };

    let point: NSPoint = unsafe {
        let raw: CgPoint = CgPoint::from(NSEvent::locationInWindow(event as id));

        let converted: CgPoint = msg_send![
            this,
            convertPoint: raw,
            fromView: std::ptr::null_mut::<AnyObject>(),
        ];
        converted.into()
    };
    let modifiers = unsafe { NSEvent::modifierFlags(event as id) };

    let position = Point { x: point.x, y: point.y };

    state.trigger_event(Event::Mouse(MouseEvent::CursorMoved {
        position,
        modifiers: make_modifiers(modifiers),
    }));
}

extern "C-unwind" fn scroll_wheel(this: *const AnyObject, _: Sel, event: *mut AnyObject) {
    let state = unsafe { WindowState::from_view(&*this) };

    let delta = unsafe {
        let x = NSEvent::scrollingDeltaX(event as id) as f32;
        let y = NSEvent::scrollingDeltaY(event as id) as f32;

        if NSEvent::hasPreciseScrollingDeltas(event as id) != NO {
            ScrollDelta::Pixels { x, y }
        } else {
            ScrollDelta::Lines { x, y }
        }
    };

    let modifiers = unsafe { NSEvent::modifierFlags(event as id) };

    state.trigger_event(Event::Mouse(MouseEvent::WheelScrolled {
        delta,
        modifiers: make_modifiers(modifiers),
    }));
}

fn get_drag_position(sender: id) -> Point {
    let point: CgPoint = unsafe { msg_send![sender as *mut AnyObject, draggingLocation] };
    Point::new(point.x, point.y)
}

fn get_drop_data(sender: id) -> DropData {
    if sender == nil {
        return DropData::None;
    }

    unsafe {
        let pasteboard: *mut AnyObject = msg_send![sender as *mut AnyObject, draggingPasteboard];
        let pboard_type = NSFilenamesPboardType as *mut AnyObject;
        let file_list: *mut AnyObject = msg_send![pasteboard, propertyListForType: pboard_type];

        if file_list.is_null() {
            return DropData::None;
        }

        let mut files = vec![];
        for i in 0..NSArray::count(file_list as id) {
            let data = NSArray::objectAtIndex(file_list as id, i);
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

extern "C-unwind" fn dragging_entered(
    this: *const AnyObject, _sel: Sel, sender: *mut AnyObject,
) -> NSUInteger {
    let state = unsafe { WindowState::from_view(&*this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender as id);

    let event = MouseEvent::DragEntered {
        position: get_drag_position(sender as id),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    on_event(&state, event)
}

extern "C-unwind" fn dragging_updated(
    this: *const AnyObject, _sel: Sel, sender: *mut AnyObject,
) -> NSUInteger {
    let state = unsafe { WindowState::from_view(&*this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender as id);

    let event = MouseEvent::DragMoved {
        position: get_drag_position(sender as id),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    on_event(&state, event)
}

extern "C-unwind" fn prepare_for_drag_operation(
    _this: *const AnyObject, _sel: Sel, _sender: *mut AnyObject,
) -> ObjcBool {
    // Always accept drag operation if we get this far
    // This function won't be called unless dragging_entered/updated
    // has returned an acceptable operation
    ObjcBool::YES
}

extern "C-unwind" fn perform_drag_operation(
    this: *const AnyObject, _sel: Sel, sender: *mut AnyObject,
) -> ObjcBool {
    let state = unsafe { WindowState::from_view(&*this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender as id);

    let event = MouseEvent::DragDropped {
        position: get_drag_position(sender as id),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    let event_status = state.trigger_event(Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(_) => ObjcBool::YES,
        _ => ObjcBool::NO,
    }
}

extern "C-unwind" fn dragging_exited(this: *const AnyObject, _sel: Sel, _sender: *mut AnyObject) {
    let state = unsafe { WindowState::from_view(&*this) };

    on_event(&state, MouseEvent::DragLeft);
}

extern "C-unwind" fn handle_notification(
    this: *const AnyObject, _cmd: Sel, notification: *mut AnyObject,
) {
    unsafe {
        let state = WindowState::from_view(&*this);

        // The subject of the notication, in this case an NSWindow object.
        let notification_object: *mut AnyObject = msg_send![notification, object];

        // The NSWindow object associated with our NSView.
        let window: *mut AnyObject = msg_send![this, window];

        let first_responder: *mut AnyObject = msg_send![window, firstResponder];

        // Only trigger focus events if the NSWindow that's being notified about is our window,
        // and if the window's first responder is our NSView.
        // If the first responder isn't our NSView, the focus events will instead be triggered
        // by the becomeFirstResponder and resignFirstResponder methods on the NSView itself.
        if notification_object == window && first_responder == this as *mut AnyObject {
            let is_key_window: ObjcBool = msg_send![window, isKeyWindow];
            state.trigger_event(Event::Window(if is_key_window.as_bool() {
                WindowEvent::Focused
            } else {
                WindowEvent::Unfocused
            }));
        }
    }
}
