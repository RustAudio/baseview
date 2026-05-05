use std::ffi::CStr;
use std::fmt::{Debug, Display, Formatter};
use x11_dl::xlib;

use crate::x11::XcbConnection;
use std::cell::RefCell;
use std::error::Error;
use std::os::raw::{c_int, c_uchar, c_ulong};
use std::panic::AssertUnwindSafe;

thread_local! {
    /// Used as part of [`XErrorHandler::handle()`]. When an X11 error occurs during this function,
    /// the error gets copied to this RefCell after which the program is allowed to resume. The
    /// error can then be converted to a regular Rust Result value afterward.
    static CURRENT_X11_ERROR: RefCell<Option<CaughtXLibError>> = const { RefCell::new(None) };
}

/// A helper struct for safe X11 error handling
pub struct XErrorHandler<'a> {
    conn: &'a XcbConnection,
    error: &'a RefCell<Option<CaughtXLibError>>,
}

impl<'a> XErrorHandler<'a> {
    /// Syncs and checks if any previous X11 calls from the given display returned an error
    pub fn check(&mut self) -> Result<(), XLibError> {
        // Flush all possible previous errors
        unsafe { (self.conn.xlib.XSync)(self.conn.dpy, 0) };

        let error = self.error.borrow_mut().take();

        match error {
            None => Ok(()),
            Some(inner) => Err(XLibError::from_inner(inner, self.conn)),
        }
    }

    /// Sets up a temporary X11 error handler for the duration of the given closure, and allows
    /// that closure to check on the latest X11 error at any time.
    pub fn handle<T, F: FnOnce(&mut XErrorHandler) -> T>(conn: &XcbConnection, handler: F) -> T {
        /// # Safety
        /// The given display and error pointers *must* be valid for the duration of this function.
        unsafe extern "C" fn error_handler(
            _dpy: *mut xlib::Display, err: *mut xlib::XErrorEvent,
        ) -> i32 {
            // SAFETY: the error pointer should be safe to access
            let err = &*err;

            CURRENT_X11_ERROR.with(|error| {
                let mut error = error.borrow_mut();
                match error.as_mut() {
                    // If multiple errors occur, keep the first one since that's likely going to be the
                    // cause of the other errors
                    Some(_) => 1,
                    None => {
                        *error = Some(CaughtXLibError::from_event(err));
                        0
                    }
                }
            })
        }

        // Flush all possible previous errors
        unsafe { (conn.xlib.XSync)(conn.dpy, 0) };

        CURRENT_X11_ERROR.with(|error| {
            // Make sure to clear any errors from the last call to this function
            {
                *error.borrow_mut() = None;
            }

            let old_handler = unsafe { (conn.xlib.XSetErrorHandler)(Some(error_handler)) };
            let panic_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut h = XErrorHandler { conn, error };
                handler(&mut h)
            }));
            // Whatever happened, restore old error handler
            unsafe { (conn.xlib.XSetErrorHandler)(old_handler) };

            match panic_result {
                Ok(v) => v,
                Err(e) => std::panic::resume_unwind(e),
            }
        })
    }
}

struct CaughtXLibError {
    type_: c_int,
    resourceid: xlib::XID,
    serial: c_ulong,
    error_code: c_uchar,
    request_code: c_uchar,
    minor_code: c_uchar,
}

impl CaughtXLibError {
    fn from_event(error: &xlib::XErrorEvent) -> CaughtXLibError {
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
    fn from_inner(inner: CaughtXLibError, conn: &XcbConnection) -> Self {
        Self { display_name: Self::get_display_name(&inner, conn), inner }
    }

    fn get_display_name(error: &CaughtXLibError, conn: &XcbConnection) -> Box<str> {
        let mut buf = [0; 255];
        unsafe {
            (conn.xlib.XGetErrorText)(
                conn.dpy,
                error.error_code.into(),
                buf.as_mut_ptr().cast(),
                (buf.len() - 1) as i32,
            )
        };

        *buf.last_mut().unwrap() = 0;
        // SAFETY: whatever XGetErrorText did or not, we guaranteed there is a nul byte at the end of the buffer
        let cstr = unsafe { CStr::from_ptr(buf.as_mut_ptr().cast()) };

        cstr.to_string_lossy().into()
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
