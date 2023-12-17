use std::collections::HashMap;
/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.
use std::ffi::{CStr, CString};

use crate::MouseCursor;

use super::cursor;

pub(crate) struct Atoms {
    pub wm_protocols: Option<u32>,
    pub wm_delete_window: Option<u32>,
}

pub struct XcbConnection {
    pub conn: xcb::Connection,
    pub xlib_display: i32,

    pub(crate) atoms: Atoms,

    pub(super) cursor_cache: HashMap<MouseCursor, u32>,
}

macro_rules! intern_atoms {
    ($conn:expr, $( $name:ident ),+ ) => {{
        $(
            #[allow(non_snake_case)]
            let $name = xcb::intern_atom($conn, true, stringify!($name));
        )+

        // splitting request and reply to improve throughput

        (
            $( $name.get_reply()
                .map(|r| r.atom())
                .ok()),+
        )
    }};
}

impl XcbConnection {
    pub fn new() -> Result<Self, xcb::base::ConnError> {
        let (conn, xlib_display) = xcb::Connection::connect_with_xlib_display()?;

        conn.set_event_queue_owner(xcb::base::EventQueueOwner::Xcb);

        let (wm_protocols, wm_delete_window) = intern_atoms!(&conn, WM_PROTOCOLS, WM_DELETE_WINDOW);

        Ok(Self {
            conn,
            xlib_display,

            atoms: Atoms { wm_protocols, wm_delete_window },

            cursor_cache: HashMap::new(),
        })
    }

    // Try to get the scaling with this function first.
    // If this gives you `None`, fall back to `get_scaling_screen_dimensions`.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_xft(&self) -> Option<f64> {
        use x11::xlib::{
            XResourceManagerString, XrmDestroyDatabase, XrmGetResource, XrmGetStringDatabase,
            XrmValue,
        };

        let display = self.conn.get_raw_dpy();
        unsafe {
            let rms = XResourceManagerString(display);
            if !rms.is_null() {
                let db = XrmGetStringDatabase(rms);
                if !db.is_null() {
                    let mut value = XrmValue { size: 0, addr: std::ptr::null_mut() };

                    let mut value_type: *mut std::os::raw::c_char = std::ptr::null_mut();
                    let name_c_str = CString::new("Xft.dpi").unwrap();
                    let c_str = CString::new("Xft.Dpi").unwrap();

                    let dpi = if XrmGetResource(
                        db,
                        name_c_str.as_ptr(),
                        c_str.as_ptr(),
                        &mut value_type,
                        &mut value,
                    ) != 0
                        && !value.addr.is_null()
                    {
                        let value_addr: &CStr = CStr::from_ptr(value.addr);
                        value_addr.to_str().ok();
                        let value_str = value_addr.to_str().ok()?;
                        let value_f64: f64 = value_str.parse().ok()?;
                        let dpi_to_scale = value_f64 / 96.0;
                        Some(dpi_to_scale)
                    } else {
                        None
                    };
                    XrmDestroyDatabase(db);

                    return dpi;
                }
            }
        }
        None
    }

    // Try to get the scaling with `get_scaling_xft` first.
    // Only use this function as a fallback.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_screen_dimensions(&self) -> Option<f64> {
        // Figure out screen information
        let setup = self.conn.get_setup();
        let screen = setup.roots().nth(self.xlib_display as usize).unwrap();

        // Get the DPI from the screen struct
        //
        // there are 2.54 centimeters to an inch; so there are 25.4 millimeters.
        // dpi = N pixels / (M millimeters / (25.4 millimeters / 1 inch))
        //     = N pixels / (M inch / 25.4)
        //     = N * 25.4 pixels / M inch
        let width_px = screen.width_in_pixels() as f64;
        let width_mm = screen.width_in_millimeters() as f64;
        let height_px = screen.height_in_pixels() as f64;
        let height_mm = screen.height_in_millimeters() as f64;
        let _xres = width_px * 25.4 / width_mm;
        let yres = height_px * 25.4 / height_mm;

        let yscale = yres / 96.0;

        // TODO: choose between `xres` and `yres`? (probably both are the same?)
        Some(yscale)
    }

    #[inline]
    pub fn get_scaling(&self) -> Option<f64> {
        self.get_scaling_xft().or_else(|| self.get_scaling_screen_dimensions())
    }

    #[inline]
    pub fn get_cursor_xid(&mut self, cursor: MouseCursor) -> u32 {
        let dpy = self.conn.get_raw_dpy();

        *self.cursor_cache.entry(cursor).or_insert_with(|| cursor::get_xcursor(dpy, cursor))
    }
}
