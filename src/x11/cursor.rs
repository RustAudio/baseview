use std::os::raw::{c_ulong, c_char};
use std::collections::HashMap;

use crate::MouseCursor;

pub fn set_cursor(
    xcb_connection: &mut crate::x11::XcbConnection,
    window_id: u32,
    cursor_cache: &mut HashMap<MouseCursor, c_ulong>,
    mouse_cursor: MouseCursor,
) {
    let display = xcb_connection.conn.get_raw_dpy();

    let cursor = *cursor_cache
                .entry(mouse_cursor)
                .or_insert_with(|| get_cursor(display, mouse_cursor));

    unsafe {
        if cursor != 0 {
            x11::xlib::XDefineCursor(display, window_id as c_ulong, cursor);
        }
        x11::xlib::XFlush(display);
    }
}

fn get_cursor(display: *mut x11::xlib::Display, cursor: MouseCursor) -> c_ulong {
    let load = |name: &[u8]| load_cursor(display, name);
    let loadn = |names: &[&[u8]]| load_first_existing_cursor(display, names);

    let mut cursor = match cursor {
        MouseCursor::Default => load(b"left_ptr\0"),
        MouseCursor::Hand => loadn(&[b"hand2\0", b"hand1\0"]),
        MouseCursor::HandGrabbing => loadn(&[b"closedhand\0", b"grabbing\0"]),
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

    if cursor == 0 {
        cursor = load(b"left_ptr\0")
    }

    cursor
}

fn load_cursor(display: *mut x11::xlib::Display, name: &[u8]) -> c_ulong {
    unsafe {
        x11::xcursor::XcursorLibraryLoadCursor(display, name.as_ptr() as *const c_char)
    }
}

fn load_first_existing_cursor(display: *mut x11::xlib::Display, names: &[&[u8]]) -> c_ulong {
    for name in names.iter() {
        let xcursor = load_cursor(display, name);
        if xcursor != 0 {
            return xcursor;
        }
    }
    0
}

fn create_empty_cursor(display: *mut x11::xlib::Display,) -> c_ulong {
    let data = 0;
    let pixmap = unsafe {
        let screen = x11::xlib::XDefaultScreen(display);
        let window = x11::xlib::XRootWindow(display, screen);
        x11::xlib::XCreateBitmapFromData(display, window, &data, 1, 1)
    };

    if pixmap == 0 {
        panic!("failed to allocate pixmap for cursor");
    }

    unsafe {
        // We don't care about this color, since it only fills bytes
        // in the pixmap which are not 0 in the mask.
        let mut dummy_color = maybe_uninit::MaybeUninit::uninit();
        let cursor = x11::xlib::XCreatePixmapCursor(
            display,
            pixmap,
            pixmap,
            dummy_color.as_mut_ptr(),
            dummy_color.as_mut_ptr(),
            0,
            0,
        );
        x11::xlib::XFreePixmap(display, pixmap);

        cursor
    }
}