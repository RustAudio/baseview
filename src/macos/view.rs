use std::ffi::c_void;

use cocoa::appkit::{NSEvent, NSView};
use cocoa::base::{id, nil, BOOL, YES, NO};
use cocoa::foundation::{NSArray, NSPoint, NSRect, NSSize};

use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use uuid::Uuid;

use crate::{Event, MouseButton, MouseEvent, Point, WindowOpenOptions};
use crate::MouseEvent::{ButtonPressed, ButtonReleased};

use super::window::WindowState;


/// Name of the field used to store the `WindowState` pointer.
pub(super) const BASEVIEW_WINDOW_STATE_IVAR: &str = "baseview_window_state";

/// Name of the field use to store retain count of view after calling
/// user-supplied `build` fn.
pub(super) const BASEVIEW_RETAIN_COUNT_IVAR: &str = "baseview_retain_count";


pub(super) unsafe fn create_view(
    window_options: &WindowOpenOptions,
) -> id {
    let class = create_view_class();

    let view: id = msg_send![class, alloc];

    let size = window_options.size;

    view.initWithFrame_(NSRect::new(
        NSPoint::new(0., 0.),
        NSSize::new(size.width, size.height),
    ));

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
        property_yes as extern "C" fn(&Object, Sel) -> BOOL
    );
    class.add_method(
        sel!(isFlipped),
        property_yes as extern "C" fn(&Object, Sel) -> BOOL
    );
    class.add_method(
        sel!(preservesContentInLiveResize),
        property_no as extern "C" fn(&Object, Sel) -> BOOL
    );
    class.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, id) -> BOOL
    );

    class.add_method(
        sel!(release),
        release as extern "C" fn(&Object, Sel)
    );
    class.add_method(
        sel!(viewWillMoveToWindow:),
        view_will_move_to_window as extern "C" fn(&Object, Sel, id)
    );
    class.add_method(
        sel!(updateTrackingAreas:),
        update_tracking_areas as extern "C" fn(&Object, Sel, id)
    );

    class.add_method(
        sel!(mouseMoved:),
        mouse_moved as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(mouseDragged:),
        mouse_moved as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(rightMouseDragged:),
        mouse_moved as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(otherMouseDragged:),
        mouse_moved as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(mouseEntered:),
        mouse_entered as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(mouseExited:),
        mouse_exited as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(mouseDown:),
        left_mouse_down as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(mouseUp:),
        left_mouse_up as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(rightMouseDown:),
        right_mouse_down as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(rightMouseUp:),
        right_mouse_up as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(otherMouseDown:),
        middle_mouse_down as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(otherMouseUp:),
        middle_mouse_up as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(keyDown:),
        key_down as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(keyUp:),
        key_up as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(flagsChanged:),
        flags_changed as extern "C" fn(&Object, Sel, id),
    );

    class.add_ivar::<*mut c_void>(BASEVIEW_WINDOW_STATE_IVAR);
    class.add_ivar::<usize>(BASEVIEW_RETAIN_COUNT_IVAR);

    class.register()
}


extern "C" fn property_yes(
    _this: &Object,
    _sel: Sel,
) -> BOOL {
    YES
}


extern "C" fn property_no(
    _this: &Object,
    _sel: Sel,
) -> BOOL {
    NO
}


extern "C" fn accepts_first_mouse(
    _this: &Object,
    _sel: Sel,
    _event: id
) -> BOOL {
    YES
}


extern "C" fn release(this: &Object, _sel: Sel) {
    unsafe {
        let superclass = msg_send![this, superclass];

        let () = msg_send![super(this, superclass), release];
    }

    unsafe {
        let retain_count: usize = msg_send![this, retainCount];
        let retain_count_after_build: usize = *(*this).get_ivar(
            BASEVIEW_RETAIN_COUNT_IVAR
        );

        if retain_count == retain_count_after_build {
            WindowState::from_field(this).remove_timer();

            // Drop WindowState
            let state_ptr: *mut c_void = *this.get_ivar(
                BASEVIEW_WINDOW_STATE_IVAR
            );
            Box::from_raw(state_ptr as *mut WindowState);
        }

        if retain_count == 1 {
            // Delete class
            let class = msg_send![this, class];
            ::objc::runtime::objc_disposeClassPair(class);
        }
    }
}


/// Init/reinit tracking area
///
/// Info:
/// https://developer.apple.com/documentation/appkit/nstrackingarea
/// https://developer.apple.com/documentation/appkit/nstrackingarea/options
/// https://developer.apple.com/documentation/appkit/nstrackingareaoptions
unsafe fn reinit_tracking_area(this: &Object, tracking_area: *mut Object){
    let options: usize = {
        let mouse_entered_and_exited = 0x01;
        let tracking_mouse_moved = 0x02;
        let tracking_cursor_update = 0x04;
        let tracking_active_in_active_app = 0x40;
        let tracking_in_visible_rect = 0x200;
        let tracking_enabled_during_mouse_drag = 0x400;

        mouse_entered_and_exited | tracking_mouse_moved |
            tracking_cursor_update | tracking_active_in_active_app |
            tracking_in_visible_rect | tracking_enabled_during_mouse_drag
    };

    let bounds: NSRect = msg_send![this, bounds];

    *tracking_area = msg_send![tracking_area,
        initWithRect:bounds
        options:options
        owner:this
        userInfo:nil
    ];
}


extern "C" fn view_will_move_to_window(
    this: &Object,
    _self: Sel,
    new_window: id
){
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area_count = NSArray::count(tracking_areas);

        let _: () = msg_send![class!(NSEvent), setMouseCoalescingEnabled:NO];

        if new_window == nil {
            if tracking_area_count != 0 {
                let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);


                let _: () = msg_send![this, removeTrackingArea:tracking_area];
                let _: () = msg_send![tracking_area, release];
            }

        } else {
            if tracking_area_count == 0 {
                let class = Class::get("NSTrackingArea").unwrap();

                let tracking_area: *mut Object = msg_send![class, alloc];

                reinit_tracking_area(this, tracking_area);

                let _: () = msg_send![this, addTrackingArea:tracking_area];
            }

            let _: () = msg_send![new_window, setAcceptsMouseMovedEvents:YES];
            let _: () = msg_send![new_window, makeFirstResponder:this];
        }
    }

    unsafe {
        let superclass = msg_send![this, superclass];

        let () = msg_send![super(this, superclass), viewWillMoveToWindow:new_window];
    }
}


extern "C" fn update_tracking_areas(
    this: &Object,
    _self: Sel,
    _: id
){
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);

        reinit_tracking_area(this, tracking_area);
    }
}


extern "C" fn mouse_moved(
    this: &Object,
    _sel: Sel,
    event: id
){
    let point: NSPoint = unsafe {
        let point = NSEvent::locationInWindow(event);

        msg_send![this, convertPoint:point fromView:nil]
    };

    let position = Point {
        x: point.x,
        y: point.y
    };

    let event = Event::Mouse(MouseEvent::CursorMoved { position });

    let state: &mut WindowState = unsafe {
        WindowState::from_field(this)
    };

    state.trigger_event(event);
}


macro_rules! mouse_simple_extern_fn {
    ($fn:ident, $event:expr) => {
        extern "C" fn $fn(
            this: &Object,
            _sel: Sel,
            _event: id,
        ){
            let state: &mut WindowState = unsafe {
                WindowState::from_field(this)
            };

            state.trigger_event(Event::Mouse($event));
        }
    };
}


mouse_simple_extern_fn!(left_mouse_down, ButtonPressed(MouseButton::Left));
mouse_simple_extern_fn!(left_mouse_up, ButtonReleased(MouseButton::Left));

mouse_simple_extern_fn!(right_mouse_down, ButtonPressed(MouseButton::Right));
mouse_simple_extern_fn!(right_mouse_up, ButtonReleased(MouseButton::Right));

mouse_simple_extern_fn!(middle_mouse_down, ButtonPressed(MouseButton::Middle));
mouse_simple_extern_fn!(middle_mouse_up, ButtonReleased(MouseButton::Middle));

mouse_simple_extern_fn!(mouse_entered, MouseEvent::CursorEntered);
mouse_simple_extern_fn!(mouse_exited, MouseEvent::CursorLeft);


extern "C" fn key_down(this: &Object, _: Sel, event: id){
    let state: &mut WindowState = unsafe {
        WindowState::from_field(this)
    };

    if let Some(key_event) = state.process_native_key_event(event){
        state.trigger_event(Event::Keyboard(key_event));
    }
}


extern "C" fn key_up(this: &Object, _: Sel, event: id){
    let state: &mut WindowState = unsafe {
        WindowState::from_field(this)
    };

    if let Some(key_event) = state.process_native_key_event(event){
        state.trigger_event(Event::Keyboard(key_event));
    }
}


extern "C" fn flags_changed(this: &Object, _: Sel, event: id){
    let state: &mut WindowState = unsafe {
        WindowState::from_field(this)
    };

    if let Some(key_event) = state.process_native_key_event(event){
        state.trigger_event(Event::Keyboard(key_event));
    }
}