#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MouseButtonID {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    Other(u8),
}

#[derive(Debug, Copy, Clone)]
pub struct MouseScroll {
    pub x_delta: f64,
    pub y_delta: f64,
}

#[derive(Debug, Copy, Clone)]
pub enum MouseClickType {
    Single,
    Double,
    Triple,
}

#[derive(Debug, Copy, Clone)]
pub struct MouseClick {
    pub id: MouseButtonID,
    pub click_type: MouseClickType,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub struct WindowInfo {
    pub width: u32,
    pub height: u32,
    pub dpi: Option<f64>,
}

#[derive(Debug)]
pub enum Event {
    RenderExpose,
    CursorMotion(i32, i32), // new (x, y) relative to window
    MouseDown(MouseButtonID),
    MouseUp(MouseButtonID),
    MouseScroll(MouseScroll),
    MouseClick(MouseClick),
    KeyDown(u8),               // keycode
    KeyUp(u8),                 // keycode
    CharacterInput(u32),       // character code
    WindowResized(WindowInfo), // new (width, height)
    WindowFocus,
    WindowUnfocus,
    WillClose,
}
