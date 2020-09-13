#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum MouseCursor {
    Default,
    Hand,
    HandGrabbing,
    Help,

    Hidden,

    Text,
    VerticalText,

    Working,
    PtrWorking,

    NotAllowed,
    PtrNotAllowed,

    ZoomIn,
    ZoomOut,

    Alias,
    Copy,
    Move,
    AllScroll,
    Cell,
    Crosshair,

    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NwseResize,
    NeswResize,
    ColResize,
    RowResize,
}

impl Default for MouseCursor {
    fn default() -> Self {
        Self::Default
    }
}
