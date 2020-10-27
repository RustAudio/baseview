use std::ffi::c_void;
use std::sync::Arc;

use cocoa::appkit::{NSEvent, NSView};
use cocoa::base::{id, nil, BOOL, YES};
use cocoa::foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};

use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};

use crate::{
    Event, MouseButton, MouseEvent, KeyboardEvent, Point, WindowHandler,
    WindowOpenOptions
};
use crate::MouseEvent::{ButtonPressed, ButtonReleased};

use super::window::{WindowState, WINDOW_STATE_IVAR_NAME};


pub(super) unsafe fn create_view<H: WindowHandler>(
    window_options: &WindowOpenOptions,
) -> id {
    let mut class = ClassDecl::new("BaseviewNSView", class!(NSView))
        .unwrap();

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
        sel!(keyDown:),
        key_down::<H> as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(keyUp:),
        key_up::<H> as extern "C" fn(&Object, Sel, id),
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

    let class = class.register();
    let view: id = msg_send![class, alloc];

    let size = window_options.size;

    view.initWithFrame_(NSRect::new(
        NSPoint::new(0., 0.),
        NSSize::new(size.width, size.height),
    ));

    view
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


/// Init/reinit tracking area
///
/// Info:
/// https://developer.apple.com/documentation/appkit/nstrackingarea
/// https://developer.apple.com/documentation/appkit/nstrackingarea/options
/// https://developer.apple.com/documentation/appkit/nstrackingareaoptions
unsafe fn reinit_tracking_area(this: &Object, tracking_area: *mut Object){
    let option_u32: u32 = {
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
        options:option_u32
        owner:this
        userInfo:nil
    ];
}


extern "C" fn view_will_move_to_window<H: WindowHandler>(this: &Object, _self: Sel, new_window: id){
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area_count = NSArray::count(tracking_areas);

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


extern "C" fn update_tracking_areas<H: WindowHandler>(this: &Object, _self: Sel, _: id){
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);

        reinit_tracking_area(this, tracking_area);
    }
}


macro_rules! key_extern_fn {
    ($fn:ident, $event_variant:expr) => {
        extern "C" fn $fn<H: WindowHandler>(this: &Object, _sel: Sel, event: id) {
            let state: &mut WindowState<H> = WindowState::from_field(this);

            let characters = unsafe {
                let ns_string = NSEvent::characters(event);

                let start = NSString::UTF8String(ns_string);
                let len = NSString::len(ns_string);

                let slice = ::std::slice::from_raw_parts(
                    start as *const u8,
                    len
                );

                ::std::str::from_utf8_unchecked(slice)
            };

            for c in characters.chars() {
                let event = Event::Keyboard($event_variant(c as u32));
                state.trigger_event(event);
            }
        }
    };
}


key_extern_fn!(key_down, KeyboardEvent::KeyPressed);
key_extern_fn!(key_up, KeyboardEvent::KeyReleased);


extern "C" fn mouse_moved<H: WindowHandler>(this: &Object, _sel: Sel, event: id) {
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


macro_rules! mouse_button_extern_fn {
    ($fn:ident, $event:expr) => {
        extern "C" fn $fn<H: WindowHandler>(this: &Object, _sel: Sel, _event: id) {
            let state: &mut WindowState<H> = WindowState::from_field(this);

            state.trigger_event(Event::Mouse($event));
        }
    };
}


mouse_button_extern_fn!(left_mouse_down, ButtonPressed(MouseButton::Left));
mouse_button_extern_fn!(left_mouse_up, ButtonReleased(MouseButton::Left));

mouse_button_extern_fn!(right_mouse_down, ButtonPressed(MouseButton::Right));
mouse_button_extern_fn!(right_mouse_up, ButtonReleased(MouseButton::Right));

mouse_button_extern_fn!(middle_mouse_down, ButtonPressed(MouseButton::Middle));
mouse_button_extern_fn!(middle_mouse_up, ButtonReleased(MouseButton::Middle));

mouse_button_extern_fn!(mouse_entered, MouseEvent::CursorEntered);
mouse_button_extern_fn!(mouse_exited, MouseEvent::CursorLeft);