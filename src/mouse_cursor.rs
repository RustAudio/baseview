#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord)]
pub enum MouseCursor {
    Idle,
    Pointer,
    Grab,
    Text,
    Crosshair,
    Working,
    Grabbing,
    ResizingHorizontally,
    ResizingVertically,
}

impl Default for MouseCursor {
    fn default() -> Self {
        Self::Idle
    }
}
