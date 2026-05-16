use std::error::Error;
use std::fmt::Formatter;
use std::ops::Deref;
use std::os::raw::c_int;
use std::ptr::NonNull;
use std::sync::Arc;
use x11_dl::xlib::{Display, Xlib};
use x11_dl::xlib_xcb::{XEventQueueOwner, Xlib_xcb};
use x11rb::xcb_ffi::XCBConnection;

/// A Xlib/XCB connection object.
///
/// This exposes both a raw Xlib display pointer, and a x11rb XCBConnection object.
///
/// This allows us to interface with the same connection using Xlib (needed for GLX, and for FFI),
/// as well as XCB (needed to preserve our sanity).
pub struct XlibXcbConnection {
    // SAFETY: Drop order matters here! We *MUST* Drop the XCBConnection first, as it essentially
    // borrows the Xlib/XCB connection
    xcb_connection: XCBConnection,
    xlib_connection: OwnedDisplayConnection,
}

impl XlibXcbConnection {
    pub fn open() -> Result<Self, Box<dyn Error>> {
        let xlib_xcb = Xlib_xcb::open()?;
        // Open the connection to the X11 server as a Xlib/XCB connection object
        let xlib_connection = OwnedDisplayConnection::open()?;
        // Set the XCB end of the Xlib/XCB connection object as the queue owner.
        // From now on, we'll use XCB (i.e. X11rb) to interface with the event queue
        xlib_connection.set_xcb_queue_owner(&xlib_xcb);

        // Extract the XCB connection object pointer from the Xlib/XCB connection object
        // SAFETY: This is always safe to call as long as the OwnedDisplayConnection is alive
        let xcb_connection =
            unsafe { (xlib_xcb.XGetXCBConnection)(xlib_connection.display.as_ptr()) };

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
        self.xlib_connection.default_screen()
    }

    pub fn xlib(&self) -> &Xlib {
        &self.xlib_connection.xlib
    }

    pub fn xlib_display(&self) -> *mut Display {
        self.xlib_connection.display.as_ptr()
    }

    pub fn xcb_connection(&self) -> &XCBConnection {
        &self.xcb_connection
    }
}

// For convenience
impl Deref for XlibXcbConnection {
    type Target = XCBConnection;

    fn deref(&self) -> &Self::Target {
        &self.xcb_connection
    }
}

/// An owned Xlib Display connection.
///
/// This type guarantees the inner display connection object to be alive and valid, as long as this
/// is alive.
///
/// It will also always close the display connection on drop.
struct OwnedDisplayConnection {
    display: NonNull<Display>,
    xlib: Arc<Xlib>,
}

impl OwnedDisplayConnection {
    pub fn open() -> Result<Self, Box<dyn Error>> {
        let xlib = Arc::new(Xlib::open()?);

        // SAFETY: It's always safe to call XOpenDisplay with a NULL display_name
        let ptr = unsafe { (xlib.XOpenDisplay)(core::ptr::null()) };

        let Some(display) = NonNull::new(ptr) else { return Err(DisplayOpenFailedError.into()) };

        Ok(Self { display, xlib })
    }

    fn set_xcb_queue_owner(&self, xlib_xcb: &Xlib_xcb) {
        // SAFETY: This type ensures the display pointer is always valid.
        unsafe {
            (xlib_xcb.XSetEventQueueOwner)(
                self.display.as_ptr(),
                XEventQueueOwner::XCBOwnsEventQueue,
            )
        }
    }

    /// Safe wrapper for XDefaultScreen
    fn default_screen(&self) -> c_int {
        // SAFETY: This type ensures the display pointer is always valid.
        unsafe { (self.xlib.XDefaultScreen)(self.display.as_ptr()) }
    }
}

impl Drop for OwnedDisplayConnection {
    fn drop(&mut self) {
        // SAFETY: This type guarantees the display pointer is valid.
        // This being `Drop` also prevents any double-free.
        unsafe { (self.xlib.XCloseDisplay)(self.display.as_ptr()) };
    }
}

#[derive(Debug)]
struct DisplayOpenFailedError;
impl std::fmt::Display for DisplayOpenFailedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to open X11 display connection: XOpenDisplay() failed")
    }
}
impl Error for DisplayOpenFailedError {}
