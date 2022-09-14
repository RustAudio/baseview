use std::os::raw::c_char;

use crate::MouseCursor;

fn create_empty_cursor(display: *mut x11::xlib::Display) -> Option<u32> {
    let data = 0;
    let pixmap = unsafe {
        let screen = x11::xlib::XDefaultScreen(display);
        let window = x11::xlib::XRootWindow(display, screen);
        x11::xlib::XCreateBitmapFromData(display, window, &data, 1, 1)
    };

    if pixmap == 0 {
        return None;
    }

    unsafe {
        // We don't care about this color, since it only fills bytes
        // in the pixmap which are not 0 in the mask.
        let mut color: x11::xlib::XColor = std::mem::zeroed();

        let cursor = x11::xlib::XCreatePixmapCursor(
            display,
            pixmap,
            pixmap,
            &mut color as *mut _,
            &mut color as *mut _,
            0,
            0,
        );
        x11::xlib::XFreePixmap(display, pixmap);

        Some(cursor as u32)
    }
}

fn load_cursor(display: *mut x11::xlib::Display, name: &[u8]) -> Option<u32> {
    let xcursor =
        unsafe { x11::xcursor::XcursorLibraryLoadCursor(display, name.as_ptr() as *const c_char) };

    if xcursor == 0 {
        None
    } else {
        Some(xcursor as u32)
    }
}

fn load_first_existing_cursor(display: *mut x11::xlib::Display, names: &[&[u8]]) -> Option<u32> {
    names
        .iter()
        .map(|name| load_cursor(display, name))
        .find(|xcursor| xcursor.is_some())
        .unwrap_or(None)
}

pub(super) fn get_xcursor(display: *mut x11::xlib::Display, cursor: MouseCursor) -> u32 {
    let load = |name: &[u8]| load_cursor(display, name);
    let loadn = |names: &[&[u8]]| load_first_existing_cursor(display, names);

    let cursor = match cursor {
        MouseCursor::Default => None, // catch this in the fallback case below

        MouseCursor::Hand => loadn(&[b"hand2\0", b"hand1\0"]),
        MouseCursor::HandGrabbing => loadn(&[b"closedhand\0", b"grabbing\0"]),
        MouseCursor::Pointer => loadn(&[b"hand2\0"]),
        MouseCursor::Help => load(b"question_arrow\0"),

        MouseCursor::Hidden => create_empty_cursor(display),

        MouseCursor::Text => loadn(&[b"text\0", b"xterm\0"]),
        MouseCursor::VerticalText => load(b"vertical-text\0"),

        MouseCursor::Working => load(b"watch\0"),
        MouseCursor::PtrWorking => load(b"left_ptr_watch\0"),

        MouseCursor::NotAllowed => load(b"crossed_circle\0"),
        MouseCursor::PtrNotAllowed => loadn(&[b"no-drop\0", b"crossed_circle\0"]),

        MouseCursor::ZoomIn => load(b"zoom-in\0"),
        MouseCursor::ZoomOut => load(b"zoom-out\0"),

        MouseCursor::Alias => load(b"link\0"),
        MouseCursor::Copy => load(b"copy\0"),
        MouseCursor::Move => load(b"move\0"),
        MouseCursor::AllScroll => load(b"all-scroll\0"),
        MouseCursor::Cell => load(b"plus\0"),
        MouseCursor::Crosshair => load(b"crosshair\0"),

        MouseCursor::EResize => load(b"right_side\0"),
        MouseCursor::NResize => load(b"top_side\0"),
        MouseCursor::NeResize => load(b"top_right_corner\0"),
        MouseCursor::NwResize => load(b"top_left_corner\0"),
        MouseCursor::SResize => load(b"bottom_side\0"),
        MouseCursor::SeResize => load(b"bottom_right_corner\0"),
        MouseCursor::SwResize => load(b"bottom_left_corner\0"),
        MouseCursor::WResize => load(b"left_side\0"),
        MouseCursor::EwResize => load(b"h_double_arrow\0"),
        MouseCursor::NsResize => load(b"v_double_arrow\0"),
        MouseCursor::NwseResize => loadn(&[b"bd_double_arrow\0", b"size_bdiag\0"]),
        MouseCursor::NeswResize => loadn(&[b"fd_double_arrow\0", b"size_fdiag\0"]),
        MouseCursor::ColResize => loadn(&[b"split_h\0", b"h_double_arrow\0"]),
        MouseCursor::RowResize => loadn(&[b"split_v\0", b"v_double_arrow\0"]),
    };

    cursor.or_else(|| load(b"left_ptr\0")).unwrap_or(0)
}
