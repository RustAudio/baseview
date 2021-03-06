use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::os::raw::c_char;

use x11::xlib::{Display, XDefaultScreen, XOpenDisplay};
use x11::xlib_xcb::XGetXCBConnection;
use xcb_sys::{
    xcb_atom_t, xcb_connection_t, xcb_get_setup, xcb_intern_atom, xcb_intern_atom_cookie_t,
    xcb_intern_atom_reply, xcb_screen_next, xcb_screen_t, xcb_setup_roots_iterator,
};

use crate::MouseCursor;

use super::cursor;

pub struct Atoms {
    pub wm_protocols: xcb_atom_t,
    pub wm_delete_window: xcb_atom_t,
    pub wm_normal_hints: xcb_atom_t,
}

/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.
pub struct XcbConnection {
    pub conn: *mut xcb_connection_t,
    pub display: *mut Display,
    pub screen: *mut xcb_screen_t,

    pub atoms: Atoms,

    pub cursor_cache: HashMap<MouseCursor, u32>,
}

unsafe fn get_screen(conn: *mut xcb_connection_t, display: *mut Display) -> *mut xcb_screen_t {
    let screen_number = XDefaultScreen(display);
    let setup = xcb_get_setup(conn);
    let mut roots_iter = xcb_setup_roots_iterator(setup);
    for _ in 0..screen_number {
        xcb_screen_next(&mut roots_iter);
    }
    roots_iter.data
}

unsafe fn intern_atom(conn: *mut xcb_connection_t, name: &[u8]) -> xcb_intern_atom_cookie_t {
    xcb_intern_atom(conn, 1, name.len() as u16, name.as_ptr() as *const c_char)
}

unsafe fn intern_atom_reply(
    conn: *mut xcb_connection_t, cookie: xcb_intern_atom_cookie_t,
) -> xcb_atom_t {
    let reply = xcb_intern_atom_reply(conn, cookie, std::ptr::null_mut());
    if reply.is_null() {
        return xcb_sys::XCB_NONE;
    }
    let atom = (*reply).atom;
    libc::free(reply as *mut c_void);
    atom
}

impl XcbConnection {
    pub fn new() -> Self {
        unsafe {
            let display = XOpenDisplay(std::ptr::null());
            let conn = XGetXCBConnection(display) as *mut xcb_connection_t;
            let screen = get_screen(conn, display);

            let wm_protocols_cookie = intern_atom(conn, b"WM_PROTOCOLS");
            let wm_delete_window_cookie = intern_atom(conn, b"WM_DELETE_WINDOW");
            let wm_normal_hints_cookie = intern_atom(conn, b"WM_NORMAL_HINTS");

            let wm_protocols = intern_atom_reply(conn, wm_protocols_cookie);
            let wm_delete_window = intern_atom_reply(conn, wm_delete_window_cookie);
            let wm_normal_hints = intern_atom_reply(conn, wm_normal_hints_cookie);

            Self {
                conn,
                display,
                screen,

                atoms: Atoms {
                    wm_protocols,
                    wm_delete_window,
                    wm_normal_hints,
                },

                cursor_cache: HashMap::new(),
            }
        }
    }

    // Try to get the scaling with this function first.
    // If this gives you `None`, fall back to `get_scaling_screen_dimensions`.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_xft(&self) -> Option<f64> {
        use x11::xlib::{
            XResourceManagerString, XrmDestroyDatabase, XrmGetResource, XrmGetStringDatabase,
            XrmValue,
        };

        unsafe {
            let rms = XResourceManagerString(self.display);
            if !rms.is_null() {
                let db = XrmGetStringDatabase(rms);
                if !db.is_null() {
                    let mut value = XrmValue {
                        size: 0,
                        addr: std::ptr::null_mut(),
                    };

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
        // Get the DPI from the screen struct
        //
        // there are 2.54 centimeters to an inch; so there are 25.4 millimeters.
        // dpi = N pixels / (M millimeters / (25.4 millimeters / 1 inch))
        //     = N pixels / (M inch / 25.4)
        //     = N * 25.4 pixels / M inch

        let screen = unsafe { &*self.screen };

        let width_px = screen.width_in_pixels as f64;
        let width_mm = screen.width_in_millimeters as f64;
        let height_px = screen.height_in_pixels as f64;
        let height_mm = screen.height_in_millimeters as f64;
        let _xres = width_px * 25.4 / width_mm;
        let yres = height_px * 25.4 / height_mm;

        let yscale = yres / 96.0;

        // TODO: choose between `xres` and `yres`? (probably both are the same?)
        Some(yscale)
    }

    #[inline]
    pub fn get_scaling(&self) -> Option<f64> {
        self.get_scaling_xft()
            .or(self.get_scaling_screen_dimensions())
    }

    #[inline]
    pub fn get_cursor_xid(&mut self, cursor: MouseCursor) -> u32 {
        let display = self.display;
        *self
            .cursor_cache
            .entry(cursor)
            .or_insert_with(|| cursor::get_xcursor(display, cursor))
    }
}
