use crate::gl::GlConfig;
use crate::platform::gl::CreationFailedError;
use crate::platform::X11Connection;
use crate::wrappers::egl::bound_api::BoundApi;
use crate::wrappers::egl::config::EglConfig;
use crate::wrappers::egl::context::EglContext;
use crate::wrappers::egl::surface::EglSurface;
use crate::wrappers::egl::{sys, Egl, EglError};
use crate::wrappers::xlib::XlibConnection;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use x11rb::protocol::xproto::Window;

struct EglDisplayInner {
    egl: Egl,
    raw: NonNull<c_void>,
    // Kept to ensure the connection isn't dropped as long as the EGL display is alive
    _connection: Rc<X11Connection>,
}

#[derive(Clone)]
pub struct EglDisplay {
    inner: Rc<EglDisplayInner>,
}

impl EglDisplay {
    pub fn choose_config(&self, config: &GlConfig) -> Result<Option<EglConfig>, EglError> {
        EglConfig::choose_config(config, self)
    }

    pub fn egl(&self) -> &Egl {
        &self.inner.egl
    }

    pub fn as_raw(&self) -> sys::EGLDisplay {
        self.inner.raw.as_ptr()
    }

    pub fn create_surface(
        &self, config: EglConfig, window: Window, gl_config: &GlConfig,
    ) -> Result<EglSurface, EglError> {
        EglSurface::create(self, config, window, gl_config)
    }

    pub fn create_context(
        &self, config: EglConfig, _bound: &BoundApi, gl_config: &GlConfig,
    ) -> Result<EglContext, EglError> {
        EglContext::create(self, config, gl_config)
    }
}

impl Drop for EglDisplayInner {
    fn drop(&mut self) {
        if let Err(e) = unsafe { self.egl.terminate_display(self.raw) } {
            crate::warn!("Failed to terminate EGL display connection: {}", e)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct EglVersion {
    pub major: sys::Int,
    pub minor: sys::Int,
}

impl Egl {
    pub fn create_display(
        &self, connection: &Rc<X11Connection>,
    ) -> Result<EglDisplay, CreationFailedError> {
        let display = self
            .create_display_basic(connection.conn.xlib_connection())
            .ok_or(CreationFailedError::EglNoDisplay)?;

        let egl = self.clone();

        let version = unsafe { egl.initialize_display(display)? };

        // Initialize DisplayInner here so its drop impl will run if code below fails
        let inner = EglDisplayInner { egl, raw: display, _connection: Rc::clone(connection) };

        if version.major != 1 || version.minor < 5 {
            return Err(CreationFailedError::EglUnsupportedVersion(version));
        }

        Ok(EglDisplay { inner: Rc::new(inner) })
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
