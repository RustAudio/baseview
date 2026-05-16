use std::error::Error;
use std::ffi::CStr;
use std::fmt::Formatter;
use std::os::raw::{c_int, c_uchar};
use std::ptr::NonNull;
use std::sync::Arc;
use x11_dl::xlib::{Display, XErrorEvent, Xlib};
use x11_dl::xlib_xcb::{XEventQueueOwner, Xlib_xcb};

/// An owned Xlib Display connection.
///
/// This type guarantees the inner display connection object to be alive and valid, as long as this
/// is alive.
///
/// It will also always close the display connection on drop.
pub struct XlibConnection {
    display: NonNull<Display>,
    xlib: Arc<Xlib>,
}

impl XlibConnection {
    pub fn open() -> Result<Self, Box<dyn Error>> {
        let xlib = Arc::new(Xlib::open()?);

        // SAFETY: It's always safe to call XOpenDisplay with a NULL display_name
        let ptr = unsafe { (xlib.XOpenDisplay)(core::ptr::null()) };

        let Some(display) = NonNull::new(ptr) else { return Err(DisplayOpenFailedError.into()) };

        Ok(Self { display, xlib })
    }

    pub fn set_xcb_queue_owner(&self, xlib_xcb: &Xlib_xcb) {
        // SAFETY: This type ensures the display pointer is always valid.
        unsafe {
            (xlib_xcb.XSetEventQueueOwner)(
                self.display.as_ptr(),
                XEventQueueOwner::XCBOwnsEventQueue,
            )
        }
    }

    /// Safe wrapper for XDefaultScreen
    pub fn default_screen(&self) -> c_int {
        // SAFETY: This type ensures the display pointer is always valid.
        unsafe { (self.xlib.XDefaultScreen)(self.display.as_ptr()) }
    }

    pub fn dpy(&self) -> *mut Display {
        self.display.as_ptr()
    }

    pub fn xlib(&self) -> &Xlib {
        &self.xlib
    }

    /// Calls XSync(0)
    pub fn sync(&self) {
        // SAFETY: This type ensures the display pointer is always valid.
        unsafe { (self.xlib.XSync)(self.display.as_ptr(), 0) };
    }

    pub fn get_error_text(&self, buf: &mut [u8], error_code: c_uchar) -> &CStr {
        if buf.is_empty() {
            return c"";
        }

        // PANIC: we just checked above that buf.len > 0
        let buf_len = buf.len() - 1;
        let Ok(buf_len) = buf_len.try_into() else {
            // Buffers should never get that big, something went horribly wrong.
            return c"";
        };

        // SAFETY: This type ensures the display pointer is always valid.
        // Moreover, the buffer pointer is guaranteed to be valid for writes for the given length, as it comes from the given mutable slice.
        unsafe {
            (self.xlib.XGetErrorText)(
                self.dpy(),
                error_code.into(),
                buf.as_mut_ptr().cast(),
                buf_len,
            )
        };

        // PANIC: we checked above that buf.len > 0
        *buf.last_mut().unwrap() = 0;

        // SAFETY: whatever XGetErrorText did or not, we guaranteed there is a nul byte at the end of the buffer
        unsafe { CStr::from_ptr(buf.as_mut_ptr().cast()) }
    }

    pub fn set_error_handler(
        &self, new_error_handler: Option<ErrorHandler>,
    ) -> Option<ErrorHandler> {
        // SAFETY: XSetErrorHandler is always safe to call as long as the function pointer is valid,
        // which this guarantees
        unsafe { (self.xlib.XSetErrorHandler)(new_error_handler) }
    }
}

type ErrorHandler = unsafe extern "C" fn(*mut Display, *mut XErrorEvent) -> c_int;

impl Drop for XlibConnection {
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
