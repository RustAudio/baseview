/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.

pub struct XcbConnection {
    pub conn: xcb::Connection,
    pub xlib_display: i32,
}

impl XcbConnection {
    pub fn new() -> Self {
        let (conn, xlib_display) = xcb::Connection::connect_with_xlib_display().unwrap();
        Self { conn, xlib_display }
    }
}
