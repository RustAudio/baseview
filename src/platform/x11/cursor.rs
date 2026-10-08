use super::prelude::*;
use crate::wrappers::xlib::XlibXcbConnection;
use crate::MouseCursor;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use x11rb::protocol::xproto::ChangeWindowAttributesAux;

pub struct CursorStateShared {
    mouse_cursor: Cell<MouseCursor>,
    cursor_cache: RefCell<HashMap<MouseCursor, u32>>,
}

impl CursorStateShared {
    pub(crate) fn set_mouse_cursor(
        &self, mouse_cursor: MouseCursor, window: &XcbWindow,
    ) -> PlatformResult<()> {
        if self.mouse_cursor.get() == mouse_cursor {
            return Ok(());
        }

        let xid = self.get_cursor(mouse_cursor, window.connection())?;

        if xid != 0 {
            window
                .connection()
                .conn
                .change_window_attributes(
                    window.id().get(),
                    &ChangeWindowAttributesAux::new().cursor(xid),
                )?
                .check()?;
        }

        self.mouse_cursor.set(mouse_cursor);

        Ok(())
    }

    #[inline]
    fn get_cursor(&self, cursor: MouseCursor, conn: &X11Connection) -> PlatformResult<Cursor> {
        // PANIC: this function is the only point where we access the cache, and we never call
        // external functions that may make a reentrant call to this function
        let mut cursor_cache = self.cursor_cache.borrow_mut();

        match cursor_cache.entry(cursor) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let cursor = get_xcursor(conn, cursor)?;
                entry.insert(cursor);
                Ok(cursor)
            }
        }
    }
}

impl CursorStateShared {
    pub fn new() -> Self {
        Self { mouse_cursor: MouseCursor::Default.into(), cursor_cache: HashMap::new().into() }
    }
}

fn create_empty_cursor(conn: &XlibXcbConnection) -> PlatformResult<Cursor> {
    let cursor_id = conn.generate_id()?;
    let pixmap_id = conn.generate_id()?;
    let root_window = conn.default_screen().root;
    conn.create_pixmap(1, pixmap_id, root_window, 1, 1)?;
    conn.create_cursor(cursor_id, pixmap_id, pixmap_id, 0, 0, 0, 0, 0, 0, 0, 0)?;
    conn.free_pixmap(pixmap_id)?;

    Ok(cursor_id)
}

#[inline(never)]
fn load_cursor(conn: &X11Connection, name: &str) -> PlatformResult<Option<Cursor>> {
    let cursor = conn.resources.cursor_handle.load_cursor(&conn.conn as &XCBConnection, name)?;
    if cursor != x11rb::NONE {
        Ok(Some(cursor))
    } else {
        Ok(None)
    }
}

#[inline(never)]
fn load_first_existing_cursor(
    conn: &X11Connection, names: &[&str],
) -> PlatformResult<Option<Cursor>> {
    for name in names {
        let cursor = load_cursor(conn, name)?;
        if cursor.is_some() {
            return Ok(cursor);
        }
    }

    Ok(None)
}

pub(crate) fn get_xcursor(conn: &X11Connection, cursor: MouseCursor) -> PlatformResult<Cursor> {
    let load = |name: &str| load_cursor(conn, name);
    let loadn = |names: &[&str]| load_first_existing_cursor(conn, names);

    let cursor = match cursor {
        MouseCursor::Default => None, // catch this in the fallback case below

        MouseCursor::Hand => loadn(&["hand2", "hand1"])?,
        MouseCursor::HandGrabbing => loadn(&["closedhand", "grabbing"])?,
        MouseCursor::Help => load("question_arrow")?,

        MouseCursor::Hidden => Some(create_empty_cursor(&conn.conn)?),

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
