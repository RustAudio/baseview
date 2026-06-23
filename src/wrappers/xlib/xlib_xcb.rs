use crate::wrappers::xlib::xlib_connection::XlibConnection;
use raw_window_handle::{DisplayHandle, XlibDisplayHandle};
use std::error::Error;
use std::ops::Deref;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::raw::c_int;
use std::ptr::NonNull;
use x11_dl::xlib_xcb::Xlib_xcb;
use x11rb::xcb_ffi::XCBConnection;

/// A Xlib/XCB connection object.
///
/// This exposes both a raw Xlib display pointer, and a x11rb XCBConnection object.
///
/// This allows us to interface with the same connection using Xlib (needed for GLX, and for FFI),
/// as well as XCB (needed to preserve our sanity).
///
/// (Note: The term Xlib/XCB means "Xlib over XCB", not "Xlib or XCB").
pub struct XlibXcbConnection {
    // SAFETY: Drop order matters here! We *MUST* Drop the XCBConnection first, as it essentially
    // borrows the Xlib/XCB connection
    xcb_connection: XCBConnection,
    xlib_connection: XlibConnection,
}

impl XlibXcbConnection {
    pub fn open() -> Result<Self, Box<dyn Error>> {
        let xlib_xcb = Xlib_xcb::open()?;
        // Open the connection to the X11 server as a Xlib/XCB connection object
        let xlib_connection = XlibConnection::open()?;
        // Set the XCB end of the Xlib/XCB connection object as the queue owner.
        // From now on, we'll use XCB (i.e. X11rb) to interface with the event queue
        xlib_connection.set_xcb_queue_owner(&xlib_xcb);

        // Extract the XCB connection object pointer from the Xlib/XCB connection object
        // SAFETY: This is always safe to call as long as the OwnedDisplayConnection is alive
        let xcb_connection = unsafe { (xlib_xcb.XGetXCBConnection)(xlib_connection.as_raw()) };

        // The XGetXCBConnection function is not documented to ever be able to return NULL.
        // Still, this is cheap to check, just in case.
        assert!(!xcb_connection.is_null());

        // Wrap the XCB connection object in a x11rb connection object
        // SAFETY: The xcb_connection pointer should be valid. We also enforce the drop order in this
        let xcb_connection =
            unsafe { XCBConnection::from_raw_xcb_connection(xcb_connection, false)? };

        Ok(Self { xcb_connection, xlib_connection })
    }

    pub fn default_screen(&self) -> c_int {
        self.xlib_connection.default_screen_index()
    }

    pub fn xcb_connection(&self) -> &XCBConnection {
        &self.xcb_connection
    }

    pub fn xlib_connection(&self) -> &XlibConnection {
        &self.xlib_connection
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        let raw_connection = self.xlib_connection.as_raw().cast();
        let Some(raw_connection) = NonNull::new(raw_connection) else { unreachable!() };
        let handle = XlibDisplayHandle::new(Some(raw_connection), self.default_screen());

        unsafe { DisplayHandle::borrow_raw(handle.into()) }
    }
}

// For convenience
impl Deref for XlibXcbConnection {
    type Target = XCBConnection;

    fn deref(&self) -> &Self::Target {
        &self.xcb_connection
    }
}

impl AsFd for XlibXcbConnection {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.xcb_connection.as_fd()
    }
}
