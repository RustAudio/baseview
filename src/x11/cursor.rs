use std::error::Error;

use x11rb::connection::Connection;
use x11rb::cursor::Handle as CursorHandle;
use x11rb::protocol::xproto::{ConnectionExt as _, Cursor};
use x11rb::xcb_ffi::XCBConnection;

use crate::MouseCursor;

fn create_empty_cursor(conn: &XCBConnection, screen: usize) -> Result<Cursor, Box<dyn Error>> {
    let cursor_id = conn.generate_id()?;
    let pixmap_id = conn.generate_id()?;
    let root_window = conn.setup().roots[screen].root;
    conn.create_pixmap(1, pixmap_id, root_window, 1, 1)?;
    conn.create_cursor(cursor_id, pixmap_id, pixmap_id, 0, 0, 0, 0, 0, 0, 0, 0)?;
    conn.free_pixmap(pixmap_id)?;

    Ok(cursor_id)
}

fn load_cursor(
    conn: &XCBConnection, cursor_handle: &CursorHandle, name: &str,
) -> Result<Option<Cursor>, Box<dyn Error>> {
    let cursor = cursor_handle.load_cursor(conn, name)?;
    if cursor != x11rb::NONE {
        Ok(Some(cursor))
    } else {
        Ok(None)
    }
}

fn load_first_existing_cursor(
    conn: &XCBConnection, cursor_handle: &CursorHandle, names: &[&str],
) -> Result<Option<Cursor>, Box<dyn Error>> {
    for name in names {
        let cursor = load_cursor(conn, cursor_handle, name)?;
        if cursor.is_some() {
            return Ok(cursor);
        }
    }

    Ok(None)
}

pub(super) fn get_xcursor(
    conn: &XCBConnection, screen: usize, cursor_handle: &CursorHandle, cursor: MouseCursor,
) -> Result<Cursor, Box<dyn Error>> {
    let load = |name: &str| load_cursor(conn, cursor_handle, name);
    let loadn = |names: &[&str]| load_first_existing_cursor(conn, cursor_handle, names);

    let cursor = match cursor {
        MouseCursor::Default => None, // catch this in the fallback case below

        MouseCursor::Hand => loadn(&["hand2", "hand1"])?,
        MouseCursor::HandGrabbing => loadn(&["closedhand", "grabbing"])?,
        MouseCursor::Help => load("question_arrow")?,

        MouseCursor::Hidden => Some(create_empty_cursor(conn, screen)?),

        MouseCursor::Text => loadn(&["text", "xterm"])?,
        MouseCursor::VerticalText => load("vertical-text")?,

        MouseCursor::Working => load("watch")?,
        MouseCursor::PtrWorking => load("left_ptr_watch")?,

        MouseCursor::NotAllowed => load("crossed_circle")?,
        MouseCursor::PtrNotAllowed => loadn(&["no-drop", "crossed_circle"])?,

        MouseCursor::ZoomIn => load("zoom-in")?,
        MouseCursor::ZoomOut => load("zoom-out")?,

        MouseCursor::Alias => load("link")?,
        MouseCursor::Copy => load("copy")?,
        MouseCursor::Move => load("move")?,
        MouseCursor::AllScroll => load("all-scroll")?,
        MouseCursor::Cell => load("plus")?,
        MouseCursor::Crosshair => load("crosshair")?,

        MouseCursor::EResize => load("right_side")?,
        MouseCursor::NResize => load("top_side")?,
        MouseCursor::NeResize => load("top_right_corner")?,
        MouseCursor::NwResize => load("top_left_corner")?,
        MouseCursor::SResize => load("bottom_side")?,
        MouseCursor::SeResize => load("bottom_right_corner")?,
        MouseCursor::SwResize => load("bottom_left_corner")?,
        MouseCursor::WResize => load("left_side")?,
        MouseCursor::EwResize => load("h_double_arrow")?,
        MouseCursor::NsResize => load("v_double_arrow")?,
        MouseCursor::NwseResize => loadn(&["bd_double_arrow", "size_bdiag"])?,
        MouseCursor::NeswResize => loadn(&["fd_double_arrow", "size_fdiag"])?,
        MouseCursor::ColResize => loadn(&["split_h", "h_double_arrow"])?,
        MouseCursor::RowResize => loadn(&["split_v", "v_double_arrow"])?,
    };

    if let Some(cursor) = cursor {
        Ok(cursor)
    } else {
        Ok(load("left_ptr")?.unwrap_or(x11rb::NONE))
    }
}
