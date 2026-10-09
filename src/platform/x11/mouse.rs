// For all the keyboard and mouse events, you can fetch
// `x`, `y`, `detail`, and `state`.
// - `x` and `y` are the position inside the window where the cursor currently is
//   when the event happened.
// - `detail` will tell you which keycode was pressed/released (for keyboard events)
//   or which mouse button was pressed/released (for mouse events).
//   For mouse events, here's what the value means (at least on my current mouse):
//      1 = left mouse button
//      2 = middle mouse button (scroll wheel)
//      3 = right mouse button
//      4 = scroll wheel up
//      5 = scroll wheel down
//      8 = lower side button ("back" button)
//      9 = upper side button ("forward" button)
//   Note that you *will* get a "button released" event for even the scroll wheel
//   events, which you can probably ignore.
// - `state` will tell you the state of the main three mouse buttons and some of
//   the keyboard modifier keys at the time of the event.
//   http://rtbo.github.io/rust-xcb/src/xcb/ffi/xproto.rs.html#445

use crate::platform::prelude::Handler;
use crate::platform::x11::keyboard::key_mods;
use crate::{Event, MouseButton, MouseEvent, ScrollDelta};
use dpi::PhysicalPosition;
use x11rb::protocol::xproto::{
    ButtonPressEvent, ButtonReleaseEvent, EnterNotifyEvent, LeaveNotifyEvent, MotionNotifyEvent,
};

pub fn handle_motion_notify(event: MotionNotifyEvent, handler: &Handler) {
    let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);

    handler.on_event(Event::Mouse(MouseEvent::CursorMoved {
        position: physical_pos.cast(),
        modifiers: key_mods(event.state),
    }));
}

pub fn handle_enter_notify(event: EnterNotifyEvent, handler: &Handler) {
    handler.on_event(Event::Mouse(MouseEvent::CursorEntered));
    // since no `MOTION_NOTIFY` event is generated when `ENTER_NOTIFY` is generated,
    // we generate a CursorMoved as well, so the mouse position from here isn't lost
    let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);
    handler.on_event(Event::Mouse(MouseEvent::CursorMoved {
        position: physical_pos.cast(),
        modifiers: key_mods(event.state),
    }));
}

pub fn handle_leave_notify(_: LeaveNotifyEvent, handler: &Handler) {
    handler.on_event(Event::Mouse(MouseEvent::CursorLeft));
}

pub fn handle_button_press(event: ButtonPressEvent, handler: &Handler) {
    match event.detail {
        4..=7 => {
            handler.on_event(Event::Mouse(MouseEvent::WheelScrolled {
                delta: match event.detail {
                    4 => ScrollDelta::Lines { x: 0.0, y: 1.0 },
                    5 => ScrollDelta::Lines { x: 0.0, y: -1.0 },
                    6 => ScrollDelta::Lines { x: -1.0, y: 0.0 },
                    7 => ScrollDelta::Lines { x: 1.0, y: 0.0 },
                    _ => unreachable!(),
                },
                modifiers: key_mods(event.state),
            }));
        }
        detail => {
            handler.on_event(Event::Mouse(MouseEvent::ButtonPressed {
                button: button_from_id(detail),
                modifiers: key_mods(event.state),
            }));
        }
    }
}

pub fn handle_button_release(event: ButtonReleaseEvent, handler: &Handler) {
    if !(4..=7).contains(&event.detail) {
        handler.on_event(Event::Mouse(MouseEvent::ButtonReleased {
            button: button_from_id(event.detail),
            modifiers: key_mods(event.state),
        }));
    }
}

fn button_from_id(id: u8) -> MouseButton {
    match id {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        8 => MouseButton::Back,
        9 => MouseButton::Forward,
        id => MouseButton::Other(id),
    }
}
