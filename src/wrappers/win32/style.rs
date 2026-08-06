use crate::WindowSettings;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Copy, Clone)]
pub struct WindowStyle {
    pub style: WINDOW_STYLE,
    pub style_ex: WINDOW_EX_STYLE,
}

impl WindowStyle {
    pub fn from_settings(settings: &WindowSettings) -> Self {
        if settings.parent.is_some() || settings.wait_for_parent {
            return Self { style: WS_CHILD, style_ex: 0 };
        }

        let mut style = Self {
            style: WS_POPUPWINDOW | WS_CAPTION | WS_MINIMIZEBOX | WS_CLIPSIBLINGS,
            style_ex: 0,
        };

        if settings.resizable {
            style.style |= WS_SIZEBOX | WS_MAXIMIZEBOX;
        }

        style
    }
}
