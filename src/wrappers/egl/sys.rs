#![allow(non_snake_case, non_camel_case_types, reason = "To match EGL function naming")]

use crate::platform::gl::CreationFailedError;
use libloading::Library;
use std::ffi::*;
use std::fmt::{Display, Formatter};

pub type Enum = c_uint;
pub type Boolean = c_uint;
pub type Int = c_int;
pub type EGLDisplay = *mut c_void;

pub type eglGetError = unsafe extern "system" fn() -> c_int;
pub type eglBindAPI = unsafe extern "system" fn(Enum) -> Boolean;
pub type eglQueryAPI = unsafe extern "system" fn() -> Enum;
pub type eglQueryString = unsafe extern "system" fn(EGLDisplay, Int) -> *const c_char;

pub const NONE: Int = 0x3038;
pub const ENUM_NONE: Enum = 0x3038;
pub const OPENGL_API: Enum = 0x30A2;
pub const FALSE: Boolean = 0;
pub const NO_DISPLAY: EGLDisplay = 0 as EGLDisplay;
pub const EXTENSIONS: Int = 0x3055;

#[derive(Debug, Copy, Clone)]
pub struct MissingSymbolError {
    name: &'static CStr,
}

impl Display for MissingSymbolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<MissingSymbolError> for CreationFailedError {
    fn from(value: MissingSymbolError) -> Self {
        Self::EGLMissingSymbol(value)
    }
}

impl From<libloading::Error> for CreationFailedError {
    fn from(value: libloading::Error) -> Self {
        Self::EGLLoadError(value)
    }
}

pub struct Functions {
    pub eglGetError: eglGetError,
    pub eglBindAPI: eglBindAPI,
    pub eglQueryAPI: eglQueryAPI,
    pub eglQueryString: eglQueryString,
}

impl Functions {
    pub unsafe fn load_from(library: &Library) -> Result<Self, CreationFailedError> {
        Ok(Self {
            eglGetError: Self::get(library, c"eglGetError")?,
            eglBindAPI: Self::get(library, c"eglBindAPI")?,
            eglQueryAPI: Self::get(library, c"eglQueryAPI")?,
            eglQueryString: Self::get(library, c"eglQueryString")?,
        })
    }

    unsafe fn get<T: Copy>(
        library: &Library, name: &'static CStr,
    ) -> Result<T, CreationFailedError> {
        let symbol = library.get::<Option<T>>(name)?;
        let symbol = symbol.lift_option().ok_or(MissingSymbolError { name })?;
        Ok(*symbol)
    }
}
