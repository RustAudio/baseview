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

#[derive(Debug)]
pub enum WindowEvent {
    Resized(WindowInfo),
    Focused,
    Unfocused,
    WillClose,
}

#[derive(Debug)]
pub enum Event {
    Mouse(MouseEvent),
    Keyboard(KeyboardEvent),
    Window(WindowEvent),
    MainEventsCleared,
}
