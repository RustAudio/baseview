use super::sys::*;
use super::*;
use crate::gl::GlConfig;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use x11rb::protocol::xproto::Window;

struct EglSurfaceInner {
    display: EglDisplay,
    raw: NonNull<c_void>,
}

#[derive(Clone)]
pub struct EglSurface {
    inner: Rc<EglSurfaceInner>,
}

impl EglSurface {
    pub(super) fn create(
        display: &EglDisplay, config: EglConfig, window: Window, gl_config: &GlConfig,
    ) -> Result<EglSurface, EglError> {
        let raw = display.egl().create_surface(display, config, window, gl_config)?;
        let inner = EglSurfaceInner { display: display.clone(), raw };

        Ok(Self { inner: Rc::new(inner) })
    }

    pub fn display(&self) -> &EglDisplay {
        &self.inner.display
    }

    pub fn as_raw(&self) -> *mut c_void {
        self.inner.raw.as_ptr()
    }

    pub fn swap_buffers(&self) -> Result<(), EglError> {
        self.display().egl().swap_buffers(self)
    }
}

impl Egl {
    fn get_surface_attribs(gl_config: &GlConfig) -> [Int; 3] {
        #[rustfmt::skip]
        let fb_attribs = [
            EGL_GL_COLORSPACE, if gl_config.srgb { EGL_GL_COLORSPACE_SRGB } else { EGL_GL_COLORSPACE_LINEAR },
            // EGL_RENDER_BUFFER: EGL_BACK_BUFFER is default
            EGL_NONE,
        ];

        fb_attribs
    }

    fn create_surface(
        &self, display: &EglDisplay, config: EglConfig, window: Window, gl_config: &GlConfig,
    ) -> Result<NonNull<c_void>, EglError> {
        let attribs = Self::get_surface_attribs(gl_config);
        let result = unsafe {
            (self.inner.functions.eglCreateWindowSurface)(
                display.as_raw(),
                config.0.as_ptr(),
                window as _,
                attribs.as_ptr(),
            )
        };

        NonNull::new(result).ok_or_else(|| EglError::from_last_error(self))
    }

    fn swap_buffers(&self, surface: &EglSurface) -> Result<(), EglError> {
        let result = unsafe {
            (self.inner.functions.eglSwapBuffers)(surface.display().as_raw(), surface.as_raw())
        };

        if result == FALSE {
            Err(EglError::from_last_error(self))
        } else {
            Ok(())
        }
    }
}
