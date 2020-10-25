use std::ffi::c_void;
use std::sync::Arc;

use cocoa::appkit::{NSEvent, NSView};
use cocoa::base::id;
use cocoa::foundation::{NSPoint, NSRect, NSSize};

use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Object, Sel},
    sel, sel_impl,
};

use crate::{Event, MouseButton, WindowHandler, WindowOpenOptions};
use crate::MouseEvent::{ButtonPressed, ButtonReleased};

use super::window::{WindowState, WINDOW_STATE_IVAR_NAME};


pub(super) unsafe fn create_view<H: WindowHandler>(
    window_options: &WindowOpenOptions,
) -> id {
    let mut class = ClassDecl::new("BaseviewNSView", class!(NSView))
        .unwrap();

    class.add_method(
        sel!(dealloc),
        dealloc::<H> as extern "C" fn(&Object, Sel)
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


extern "C" fn mouse_moved<H: WindowHandler>(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { NSEvent::locationInWindow(event) };
    let state: &mut WindowState<H> = WindowState::from_field(this);

    state.trigger_cursor_moved(location);
}


macro_rules! mouse_button_extern_fn {
    ($fn:ident, $event:expr) => {
        extern "C" fn $fn<H: WindowHandler>(this: &Object, _sel: Sel, event: id) {
            let location = unsafe { NSEvent::locationInWindow(event) };
            let state: &mut WindowState<H> = WindowState::from_field(this);

            state.trigger_cursor_moved(location);

            let event = Event::Mouse($event);
            state.window_handler.on_event(&mut state.window, event);
        }
    };
}


mouse_button_extern_fn!(left_mouse_down, ButtonPressed(MouseButton::Left));
mouse_button_extern_fn!(left_mouse_up, ButtonReleased(MouseButton::Left));

mouse_button_extern_fn!(right_mouse_down, ButtonPressed(MouseButton::Right));
mouse_button_extern_fn!(right_mouse_up, ButtonReleased(MouseButton::Right));

mouse_button_extern_fn!(middle_mouse_down, ButtonPressed(MouseButton::Middle));
mouse_button_extern_fn!(middle_mouse_up, ButtonReleased(MouseButton::Middle));