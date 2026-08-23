use super::*;
use crate::gl::{GlConfig, Profile};
use crate::wrappers::egl::sys::*;
use std::ptr::NonNull;

pub struct EglContext {
    display: EglDisplay,
    raw: NonNull<c_void>,
}

impl EglContext {
    pub(super) fn create(
        display: &EglDisplay, config: EglConfig, gl_config: &GlConfig,
    ) -> Result<EglContext, EglError> {
        let raw = display.egl().create_context(display, config, gl_config)?;

        Ok(Self { display: display.clone(), raw })
    }

    pub fn make_current(&self, surface: &EglSurface) -> Result<(), EglError> {
        self.display.egl().make_current(surface, self)
    }

    pub fn make_not_current(&self, _bound_api: &BoundApi) -> Result<(), EglError> {
        self.display.egl().make_not_current(self)
    }
}

impl Egl {
    fn get_context_attribs(gl_config: &GlConfig) -> [Int; 7] {
        let profile_mask = match gl_config.profile {
            Profile::Core => EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,
            Profile::Compatibility => EGL_CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT,
        };

        #[rustfmt::skip]
        let fb_attribs = [
            EGL_CONTEXT_MAJOR_VERSION, gl_config.version.0.into(),
            EGL_CONTEXT_MINOR_VERSION, gl_config.version.1.into(),
            EGL_CONTEXT_OPENGL_PROFILE_MASK, profile_mask,
            EGL_NONE,
        ];

        fb_attribs
    }

    fn create_context(
        &self, display: &EglDisplay, config: EglConfig, gl_config: &GlConfig,
    ) -> Result<NonNull<c_void>, EglError> {
        let attribs = Self::get_context_attribs(gl_config);
        let result = unsafe {
            (self.inner.functions.eglCreateContext)(
                display.as_raw(),
                config.0.as_ptr(),
                core::ptr::null_mut(),
                attribs.as_ptr(),
            )
        };

        NonNull::new(result).ok_or_else(|| EglError::from_last_error(display.egl()))
    }

    fn make_current(&self, surface: &EglSurface, context: &EglContext) -> Result<(), EglError> {
        let result = unsafe {
            (self.inner.functions.eglMakeCurrent)(
                surface.display().as_raw(),
                surface.as_raw(),
                surface.as_raw(),
                context.raw.as_ptr(),
            )
        };

        if result == FALSE {
            Err(EglError::from_last_error(self))
        } else {
            Ok(())
        }
    }

    fn make_not_current(&self, context: &EglContext) -> Result<(), EglError> {
        let current_context = unsafe { (self.inner.functions.eglGetCurrentContext)() };
        if current_context != context.raw.as_ptr() {
            return Ok(());
        }

        let result = unsafe {
            (self.inner.functions.eglMakeCurrent)(
                context.display.as_raw(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };

        if result == FALSE {
            Err(EglError::from_last_error(self))
        } else {
            Ok(())
        }
    }
}
