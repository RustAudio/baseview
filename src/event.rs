use keyboard_types::KeyboardEvent;

use crate::{WindowInfo, Point};


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    Other(u8),
}

/// A scroll movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDelta {
    /// A line-based scroll movement
    Lines {
        /// The number of horizontal lines scrolled
        x: f32,

        /// The number of vertical lines scrolled
        y: f32,
    },
    /// A pixel-based scroll movement
    Pixels {
        /// The number of horizontal pixels scrolled
        x: f32,
        /// The number of vertical pixels scrolled
        y: f32,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MouseClick {
    pub button: MouseButton,
    pub click_count: usize,
    /// The logical coordinates of the mouse position
    pub position: Point,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEvent {
    /// The mouse cursor was moved
    CursorMoved {
        /// The logical coordinates of the mouse position
        position: Point,
    },

    /// A mouse button was pressed.
    ButtonPressed(MouseButton),

    /// A mouse button was released.
    ButtonReleased(MouseButton),

    /// A mouse button was clicked.
    Click(MouseClick),

    /// The mouse wheel was scrolled.
    WheelScrolled(ScrollDelta),

    /// The mouse cursor entered the window.
    CursorEntered,

    /// The mouse cursor left the window.
    CursorLeft,
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    Resized(WindowInfo),
    Focused,
    Unfocused,
    WillClose,
}

#[derive(Debug, Clone)]
pub enum Event {
    Mouse(MouseEvent),
    Keyboard(KeyboardEvent),
    Window(WindowEvent),
}


/// Return value for [WindowHandler::on_event](`crate::WindowHandler::on_event()`),
/// indicating whether the event was handled by your window or should be passed
/// back to the platform.
///
/// For most event types, this value won't have any effect. This is the case
/// when there is no clear meaning of passing back the event to the platform,
/// or it isn't obviously useful. Currently, only [`Event::Keyboard`] variants
/// support passing back the underlying event to the platform.
#[derive(Debug)]
pub enum EventStatus {
    /// Event was handled by your window and will not be sent back to the
    /// platform for further processing.
    Captured,
    /// Event was **not** handled by your window, so pass it back to the
    /// platform. For parented windows, this usually means that the parent
    /// window will receive the event. This is useful for cases such as using
    /// DAW functionality for playing piano keys with the keyboard while a
    /// plugin window is in focus.
    Ignored,
}
