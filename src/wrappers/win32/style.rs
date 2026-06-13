use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Copy, Clone)]
pub struct WindowStyle {
    pub style: WINDOW_STYLE,
    pub style_ex: WINDOW_EX_STYLE,
}

impl WindowStyle {
    pub const fn parented() -> Self {
        Self { style: WS_CHILD | WS_VISIBLE, style_ex: 0 }
    }

    pub const fn embedded() -> Self {
        Self {
            style: WS_POPUPWINDOW
                | WS_CAPTION
                | WS_VISIBLE
                | WS_SIZEBOX
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_CLIPSIBLINGS,
            style_ex: 0,
        }
    }
}
