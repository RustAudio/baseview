use super::*;
use crate::wrappers::egl::sys::*;
use std::error::Error;
use std::ffi::c_int;
use std::fmt::Display;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EglError {
    code: c_int,
}

impl EglError {
    pub fn from_last_error(egl: &Egl) -> EglError {
        let code = unsafe { (egl.inner.functions.eglGetError)() };
        Self { code }
    }
}

impl Display for EglError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.code == EGL_SUCCESS {
            f.write_str("EGL call failed but error code is EGL_SUCCESS")
        } else {
            write!(f, "EGL error code: {:x}", self.code)
        }
    }
}

impl Error for EglError {}
