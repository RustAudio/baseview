use std::cell::RefCell;
use std::collections::hash_map::{Entry, HashMap};
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::cursor::Handle as CursorHandle;
use x11rb::protocol::xproto::{self, Cursor, Screen};
use x11rb::resource_manager;

use super::cursor;
use crate::platform::*;
use crate::wrappers::xlib::XlibXcbConnection;
use crate::MouseCursor;

mod get_property;
pub use get_property::GetPropertyError;

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,

        // Drag-N-Drop Atoms
        XdndAware,
        XdndEnter,
        XdndLeave,
        XdndDrop,
        XdndPosition,
        XdndStatus,
        XdndSelection,
        XdndFinished,
        XdndActionPrivate,
        XdndActionCopy,
        XdndActionMove,
        XdndActionLink,
        XdndActionAsk,
        XdndTypeList,
        TextUriList: b"text/uri-list",
        None: b"None",
    }
}

/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.
pub struct X11Connection {
    pub(crate) conn: Arc<XlibXcbConnection>,
    pub(crate) atoms: Atoms,
    pub(crate) resources: resource_manager::Database,
    pub(crate) cursor_handle: CursorHandle,
    pub(crate) cursor_cache: RefCell<HashMap<MouseCursor, u32>>,
}

impl X11Connection {
    pub fn new() -> Result<Self> {
        let conn = XlibXcbConnection::open()?;
        let screen = conn.default_screen();
        let xcb_conn = conn.xcb_connection();

        let atoms = Atoms::new(xcb_conn)?.reply()?;
        let resources = resource_manager::new_from_default(xcb_conn)?;
        let cursor_handle = CursorHandle::new(xcb_conn, screen as usize, &resources)?.reply()?;

        Ok(Self {
            conn: Arc::new(conn),
            atoms,
            resources,
            cursor_handle,
            cursor_cache: RefCell::new(HashMap::new()),
        })
    }

    pub fn get_scaling(&self) -> f64 {
        // If the WM didn't set any display scaling, assume a scaling factor of 1.0 (i.e. don't do any scaling)
        if let Ok(Some(dpi)) = self.resources.get_value::<u32>("Xft.dpi", "") {
            dpi as f64 / 96.0
        } else {
            1.0
        }
    }

    #[inline]
    pub fn get_cursor(&self, cursor: MouseCursor) -> Result<Cursor> {
        // PANIC: this function is the only point where we access the cache, and we never call
        // external functions that may make a reentrant call to this function
        let mut cursor_cache = self.cursor_cache.borrow_mut();

        match cursor_cache.entry(cursor) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let cursor = cursor::get_xcursor(
                    &self.conn,
                    self.conn.default_screen() as usize,
                    &self.cursor_handle,
                    cursor,
                )?;
                entry.insert(cursor);
                Ok(cursor)
            }
        }
    }

    pub fn screen(&self) -> &Screen {
        &self.conn.setup().roots[self.conn.default_screen() as usize]
    }

    pub fn get_property<T: bytemuck::Pod>(
        &self, window: xproto::Window, property: xproto::Atom, property_type: xproto::Atom,
    ) -> core::result::Result<Vec<T>, GetPropertyError> {
        get_property::get_property(window, property, property_type, &self.conn)
    }
}
