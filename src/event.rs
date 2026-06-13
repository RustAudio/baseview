use std::path::PathBuf;

use keyboard_types::{KeyboardEvent, Modifiers};

use crate::{Point, WindowInfo};

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

#[derive(Debug, Clone, PartialEq)]
pub enum MouseEvent {
    /// The mouse cursor was moved
    CursorMoved {
        /// The logical coordinates of the mouse position
        position: Point,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
    },

    /// A mouse button was pressed.
    ButtonPressed {
        /// The button that was pressed.
        button: MouseButton,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
    },

    /// A mouse button was released.
    ButtonReleased {
        /// The button that was released.
        button: MouseButton,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
    },

    /// The mouse wheel was scrolled.
    WheelScrolled {
        /// How much was scrolled, in factional lines.
        delta: ScrollDelta,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
    },

    /// The mouse cursor entered the window.
    ///
    /// May not be available on all platforms.
    CursorEntered,

    /// The mouse cursor left the window.
    ///
    /// May not be available on all platforms.
    CursorLeft,

    DragEntered {
        /// The logical coordinates of the mouse position
        position: Point,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
        /// Data being dragged
        data: DropData,
    },

    DragMoved {
        /// The logical coordinates of the mouse position
        position: Point,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
        /// Data being dragged
        data: DropData,
    },

    DragLeft,

    DragDropped {
        /// The logical coordinates of the mouse position
        position: Point,
        /// The modifiers that were held down just before the event.
        modifiers: Modifiers,
        /// Data being dragged
        data: DropData,
    },
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropEffect {
    Copy,
    Move,
    Link,
    Scroll,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DropData {
    None,
    Files(Vec<PathBuf>),
}

/// Return value for [WindowHandler::on_event](`crate::WindowHandler::on_event()`),
/// indicating whether the event was handled by your window or should be passed
/// back to the platform.
///
/// For most event types, this value won't have any effect. This is the case
/// when there is no clear meaning of passing back the event to the platform,
/// or it isn't obviously useful. Currently, only [`Event::Keyboard`] variants
/// are supported.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// We are prepared to handle the data in the drag and dropping will
    /// result in [DropEffect]
    AcceptDrop(DropEffect),
}
