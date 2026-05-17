use std::fmt::{Debug, Display, Formatter};
use x11_dl::xlib;

use super::xlib_connection::XlibConnection;
use std::cell::Cell;
use std::error::Error;
use std::os::raw::{c_int, c_uchar, c_ulong};
use std::panic::AssertUnwindSafe;

thread_local! {
    /// Used as part of [`XErrorHandler::handle()`]. When an X11 error occurs during this function,
    /// the error gets copied to this Cell after which the program is allowed to resume. The
    /// error can then be converted to a regular Rust Result value afterward.
    static CURRENT_X11_ERROR: Cell<Option<CaughtXLibError>> = const { Cell::new(None) };
}

/// A helper struct for safe X11 error handling
pub struct XErrorHandler<'a> {
    conn: &'a XlibConnection,
    error: &'a Cell<Option<CaughtXLibError>>,
}

impl<'a> XErrorHandler<'a> {
    /// Syncs and checks if any previous X11 calls from the given display returned an error
    pub fn check(&self) -> Result<(), XLibError> {
        // Flush all possible previous errors
        self.conn.sync();

        let error = self.error.take();

        match error {
            None => Ok(()),
            Some(inner) => Err(XLibError::from_inner(inner, self.conn)),
        }
    }

    /// Sets up a temporary X11 error handler for the duration of the given closure, and allows
    /// that closure to check on the latest X11 error at any time.
    pub fn handle<T, F: FnOnce(&mut XErrorHandler) -> T>(conn: &XlibConnection, handler: F) -> T {
        /// # Safety
        /// The given display and error pointers *must* be valid for the duration of this function.
        unsafe extern "C" fn error_handler(
            _dpy: *mut xlib::Display, err: *mut xlib::XErrorEvent,
        ) -> i32 {
            // SAFETY: the error pointer should always be valid for reads
            let err = unsafe { err.read() };

            CURRENT_X11_ERROR.with(|error| {
                match error.get() {
                    // If multiple errors occur, keep the first one since that's likely going to be the
                    // cause of the other errors
                    Some(_) => 1,
                    None => {
                        error.set(Some(CaughtXLibError::from_event(err)));
                        0
                    }
                }
            })
        }

        // Flush all possible previous errors
        conn.sync();

        CURRENT_X11_ERROR.with(|error| {
            // Make sure to clear any errors from the last call to this function
            error.set(None);

            let old_handler = conn.set_error_handler(Some(error_handler));
            let panic_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut h = XErrorHandler { conn, error };
                handler(&mut h)
            }));
            // Whatever happened, restore old error handler
            conn.set_error_handler(old_handler);

            match panic_result {
                Ok(v) => v,
                Err(e) => std::panic::resume_unwind(e),
            }
        })
    }
}

#[derive(Copy, Clone)]
struct CaughtXLibError {
    type_: c_int,
    resourceid: xlib::XID,
    serial: c_ulong,
    error_code: c_uchar,
    request_code: c_uchar,
    minor_code: c_uchar,
}

impl CaughtXLibError {
    fn from_event(error: xlib::XErrorEvent) -> CaughtXLibError {
        Self {
            type_: error.type_,
            resourceid: error.resourceid,
            serial: error.serial,

            error_code: error.error_code,
            request_code: error.request_code,
            minor_code: error.minor_code,
        }
    }
}

pub struct XLibError {
    inner: CaughtXLibError,
    display_name: Box<str>,
}

impl XLibError {
    fn from_inner(inner: CaughtXLibError, conn: &XlibConnection) -> Self {
        let mut buf = [0; 255];
        let cstr = conn.get_error_text(&mut buf, inner.error_code);

        Self { display_name: cstr.to_string_lossy().into(), inner }
    }
}

impl Debug for XLibError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XLibError")
            .field("error_code", &self.inner.error_code)
            .field("error_message", &self.display_name)
            .field("minor_code", &self.inner.minor_code)
            .field("request_code", &self.inner.request_code)
            .field("type", &self.inner.type_)
            .field("resource_id", &self.inner.resourceid)
            .field("serial", &self.inner.serial)
            .finish()
    }
}

impl Display for XLibError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "XLib error: {} (error code {})", &self.display_name, self.inner.error_code)
    }
}

impl Error for XLibError {}
