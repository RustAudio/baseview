use crate::KeyCode;
use std::path::PathBuf;

/// The current state of the keyboard modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifiersState {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub logo: bool,
}

impl ModifiersState {
    /// Returns true if the current [`ModifiersState`] has at least the same
    /// modifiers enabled as the given value, and false otherwise.
    ///
    /// [`ModifiersState`]: struct.ModifiersState.html
    pub fn matches(&self, modifiers: ModifiersState) -> bool {
        let shift = !modifiers.shift || self.shift;
        let control = !modifiers.control || self.control;
        let alt = !modifiers.alt || self.alt;
        let logo = !modifiers.logo || self.logo;

        shift && control && alt && logo
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyboardEvent {
    KeyPressed {
        key_code: KeyCode,
        modifiers: ModifiersState,
    },
    KeyReleased {
        key_code: KeyCode,
        modifiers: ModifiersState,
    },
    CharacterInput(char),
    ModifiersChanged(ModifiersState),
}

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
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEvent {
    /// The mouse cursor was moved
    CursorMoved {
        /// The X coordinate of the mouse position
        x: f32,
        /// The Y coordinate of the mouse position
        y: f32,
    },

    /// A mouse button was pressed.
    ButtonPressed(MouseButton),

    /// A mouse button was released.
    ButtonReleased(MouseButton),

    Click(MouseClick),

    /// The mouse wheel was scrolled.
    WheelScrolled {
        /// The scroll movement.
        delta: ScrollDelta,
    },

    /// The mouse cursor entered the window.
    CursorEntered,

    /// The mouse cursor left the window.
    CursorLeft,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WindowInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowEvent {
    Resized(WindowInfo),
    Focused,
    Unfocused,
    WillClose,
}

#[derive(PartialEq, Clone, Debug)]
pub enum FileDropEvent {
    /// A file is being hovered over the window.
    ///
    /// When the user hovers multiple files at once, this event will be emitted
    /// for each file separately.
    FileHovered(PathBuf),

    /// A file has beend dropped into the window.
    ///
    /// When the user drops multiple files at once, this event will be emitted
    /// for each file separately.
    FileDropped(PathBuf),

    /// A file was hovered, but has exited the window.
    ///
    /// There will be a single `FilesHoveredLeft` event triggered even if
    /// multiple files were hovered.
    FilesHoveredLeft,
}

#[derive(Debug)]
pub enum Event {
    Interval(f64), // delta time passed
    Mouse(MouseEvent),
    Keyboard(KeyboardEvent),
    Window(WindowEvent),
    FileDrop(FileDropEvent),
    Clipboard(Option<String>),
}
