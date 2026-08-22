use super::*;
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
        todo!()
    }
}

impl Error for EglError {}
