use crate::wrappers::egl::{sys, Egl, EglError};
use crate::wrappers::xlib::{XlibConnection, XlibXcbConnection};
use std::ffi::c_void;
use std::ptr::NonNull;

pub struct EglDisplay {
    egl: Egl,
    raw: NonNull<c_void>,
}

impl EglDisplay {
    pub(super) fn new(egl: &Egl, connection: &XlibXcbConnection) -> Result<Self, EglError> {
        let display = egl.create_display_basic(connection.xlib_connection()).unwrap();

        unsafe { egl.initialize_display(display)? };
        Ok(Self { egl: egl.clone(), raw: display })
    }
}

impl Drop for EglDisplay {
    fn drop(&mut self) {
        if let Err(e) = unsafe { self.egl.terminate_display(self.raw) } {
            crate::warn!("Failed to terminate EGL display connection: {}", e)
        }
    }
}

struct EglVersion {
    major: sys::Int,
    minor: sys::Int,
}

impl Egl {
    pub fn create_display(&self, connection: &XlibXcbConnection) -> Result<EglDisplay, EglError> {
        EglDisplay::new(self, connection)
    }

    fn create_display_basic(&self, connection: &XlibConnection) -> Option<NonNull<c_void>> {
        let result = unsafe { (self.inner.functions.eglGetDisplay)(connection.as_raw().cast()) };
        NonNull::new(result)
    }

    unsafe fn initialize_display(&self, raw: NonNull<c_void>) -> Result<EglVersion, EglError> {
        let mut version = EglVersion { major: 0, minor: 0 };

        let result = unsafe {
            (self.inner.functions.eglInitialize)(
                raw.as_ptr(),
                &mut version.major,
                &mut version.minor,
            )
        };

        if result == sys::FALSE {
            return Err(EglError::from_last_error(self));
        }

        dbg!(version.major, version.minor);

        Ok(version)
    }

    unsafe fn terminate_display(&self, raw: NonNull<c_void>) -> Result<(), EglError> {
        let result = unsafe { (self.inner.functions.eglTerminate)(raw.as_ptr()) };

        if result == sys::FALSE {
            return Err(EglError::from_last_error(self));
        }

        Ok(())
    }
}
