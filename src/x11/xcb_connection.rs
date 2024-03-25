use std::collections::hash_map::{Entry, HashMap};
use std::error::Error;

use x11::{xlib, xlib::Display, xlib_xcb};

use x11rb::connection::Connection;
use x11rb::cursor::Handle as CursorHandle;
use x11rb::protocol::xproto::Cursor;
use x11rb::resource_manager;
use x11rb::xcb_ffi::XCBConnection;

use crate::MouseCursor;

use super::cursor;

x11rb::atom_manager! {
    pub Atoms2: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
    }
}

/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.
pub struct XcbConnection {
    pub(crate) dpy: *mut Display,
    pub(crate) conn2: XCBConnection,
    pub(crate) screen: usize,
    pub(crate) atoms2: Atoms2,
    pub(crate) resources: resource_manager::Database,
    pub(crate) cursor_handle: CursorHandle,
    pub(super) cursor_cache: HashMap<MouseCursor, u32>,
}

impl XcbConnection {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let dpy = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
        assert!(!dpy.is_null());
        let xcb_connection = unsafe { xlib_xcb::XGetXCBConnection(dpy) };
        assert!(!xcb_connection.is_null());
        let screen = unsafe { xlib::XDefaultScreen(dpy) } as usize;
        let conn2 = unsafe { XCBConnection::from_raw_xcb_connection(xcb_connection, true)? };
        unsafe {
            xlib_xcb::XSetEventQueueOwner(dpy, xlib_xcb::XEventQueueOwner::XCBOwnsEventQueue)
        };

        let atoms2 = Atoms2::new(&conn2)?.reply()?;
        let resources = resource_manager::new_from_default(&conn2)?;
        let cursor_handle = CursorHandle::new(&conn2, screen, &resources)?.reply()?;

        Ok(Self {
            dpy,
            conn2,
            screen,
            atoms2,
            resources,
            cursor_handle,
            cursor_cache: HashMap::new(),
        })
    }

    // Try to get the scaling with this function first.
    // If this gives you `None`, fall back to `get_scaling_screen_dimensions`.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_xft(&self) -> Result<Option<f64>, Box<dyn Error>> {
        if let Some(dpi) = self.resources.get_value::<u32>("Xft.dpi", "")? {
            Ok(Some(dpi as f64 / 96.0))
        } else {
            Ok(None)
        }
    }

    // Try to get the scaling with `get_scaling_xft` first.
    // Only use this function as a fallback.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_screen_dimensions(&self) -> f64 {
        // Figure out screen information
        let setup = self.conn2.setup();
        let screen = &setup.roots[self.screen];

        // Get the DPI from the screen struct
        //
        // there are 2.54 centimeters to an inch; so there are 25.4 millimeters.
        // dpi = N pixels / (M millimeters / (25.4 millimeters / 1 inch))
        //     = N pixels / (M inch / 25.4)
        //     = N * 25.4 pixels / M inch
        let width_px = screen.width_in_pixels as f64;
        let width_mm = screen.width_in_millimeters as f64;
        let height_px = screen.height_in_pixels as f64;
        let height_mm = screen.height_in_millimeters as f64;
        let _xres = width_px * 25.4 / width_mm;
        let yres = height_px * 25.4 / height_mm;

        let yscale = yres / 96.0;

        // TODO: choose between `xres` and `yres`? (probably both are the same?)
        yscale
    }

    #[inline]
    pub fn get_scaling(&self) -> Result<f64, Box<dyn Error>> {
        Ok(self.get_scaling_xft()?.unwrap_or(self.get_scaling_screen_dimensions()))
    }

    #[inline]
    pub fn get_cursor(&mut self, cursor: MouseCursor) -> Result<Cursor, Box<dyn Error>> {
        match self.cursor_cache.entry(cursor) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let cursor =
                    cursor::get_xcursor(&self.conn2, self.screen, &self.cursor_handle, cursor)?;
                entry.insert(cursor);
                Ok(cursor)
            }
        }
    }
}
