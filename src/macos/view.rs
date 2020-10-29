use std::ffi::c_void;
use std::sync::Arc;

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
use once_cell::sync::OnceCell;
use uuid::Uuid;

use crate::{
    Event, MouseButton, MouseEvent, Point, WindowHandler,
    WindowOpenOptions
};
use crate::MouseEvent::{ButtonPressed, ButtonReleased};

use super::window::{WindowState, WINDOW_STATE_IVAR_NAME};


// Store class here so that it doesn't need to be recreated each time the
// editor window is opened, while still allowing the name to be randomized.
static VIEW_CLASS: OnceCell<&'static Class> = OnceCell::new();


pub(super) unsafe fn create_view<H: WindowHandler>(
    window_options: &WindowOpenOptions,
) -> id {
    let class = *VIEW_CLASS.get_or_init(|| {
        create_view_class::<H>()
    });

    let view: id = msg_send![class, alloc];

    let size = window_options.size;

    view.initWithFrame_(NSRect::new(
        NSPoint::new(0., 0.),
        NSSize::new(size.width, size.height),
    ));

    view
}


unsafe fn create_view_class<H: WindowHandler>() -> &'static Class {
    // Use unique class names to make sure that differing class definitions in
    // plugins compiled with different versions of baseview don't cause issues.
    let class_name = format!("BaseviewNSView_{}", Uuid::new_v4().to_simple());
    let mut class = ClassDecl::new(&class_name, class!(NSView)).unwrap();

    class.add_method(
        sel!(acceptsFirstResponder),
        property_yes::<H> as extern "C" fn(&Object, Sel) -> BOOL
    );
    class.add_method(
        sel!(isFlipped),
        property_yes::<H> as extern "C" fn(&Object, Sel) -> BOOL
    );
    class.add_method(
        sel!(preservesContentInLiveResize),
        property_no::<H> as extern "C" fn(&Object, Sel) -> BOOL
    );
    class.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse::<H> as extern "C" fn(&Object, Sel, id) -> BOOL
    );

    class.add_method(
        sel!(dealloc),
        dealloc::<H> as extern "C" fn(&Object, Sel)
    );
    class.add_method(
        sel!(viewWillMoveToWindow:),
        view_will_move_to_window::<H> as extern "C" fn(&Object, Sel, id)
    );
    class.add_method(
        sel!(updateTrackingAreas:),
        update_tracking_areas::<H> as extern "C" fn(&Object, Sel, id)
    );

    class.add_method(
        sel!(mouseMoved:),
        mouse_moved::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(mouseDragged:),
        mouse_moved::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(rightMouseDragged:),
        mouse_moved::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(otherMouseDragged:),
        mouse_moved::<H> as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(mouseEntered:),
        mouse_entered::<H> as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(mouseExited:),
        mouse_exited::<H> as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(mouseDown:),
        left_mouse_down::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(mouseUp:),
        left_mouse_up::<H> as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(rightMouseDown:),
        right_mouse_down::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(rightMouseUp:),
        right_mouse_up::<H> as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(otherMouseDown:),
        middle_mouse_down::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(otherMouseUp:),
        middle_mouse_up::<H> as extern "C" fn(&Object, Sel, id),
    );

    class.add_ivar::<*mut c_void>(WINDOW_STATE_IVAR_NAME);

    class.register()
}


extern "C" fn dealloc<H: WindowHandler>(this: &Object, _sel: Sel) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar(WINDOW_STATE_IVAR_NAME);
        Arc::from_raw(state_ptr as *mut WindowState<H>);
    }
}


extern "C" fn property_yes<H: WindowHandler>(
    _this: &Object,
    _sel: Sel,
) -> BOOL {
    YES
}


extern "C" fn property_no<H: WindowHandler>(
    _this: &Object,
    _sel: Sel,
) -> BOOL {
    YES
}


extern "C" fn accepts_first_mouse<H: WindowHandler>(
    _this: &Object,
    _sel: Sel,
    _event: id
) -> BOOL {
    YES
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


extern "C" fn view_will_move_to_window<H: WindowHandler>(
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
        let superview: &Object = msg_send![this, superview];

        let _: () = msg_send![superview, viewWillMoveToWindow:new_window];
    }
}


extern "C" fn update_tracking_areas<H: WindowHandler>(
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


extern "C" fn mouse_moved<H: WindowHandler>(
    this: &Object,
    _sel: Sel,
    event: id
){
    let state: &mut WindowState<H> = WindowState::from_field(this);

    let point: NSPoint = unsafe {
        let point = NSEvent::locationInWindow(event);

        msg_send![this, convertPoint:point fromView:nil]
    };

    let position = Point {
        x: point.x,
        y: point.y
    };

    let event = Event::Mouse(MouseEvent::CursorMoved { position });

    state.trigger_event(event);
}


macro_rules! mouse_simple_extern_fn {
    ($fn:ident, $event:expr) => {
        extern "C" fn $fn<H: WindowHandler>(
            this: &Object,
            _sel: Sel,
            _event: id,
        ){
            let state: &mut WindowState<H> = WindowState::from_field(this);

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