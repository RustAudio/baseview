#![allow(non_snake_case, non_camel_case_types, reason = "To match EGL function naming")]

use crate::platform::gl::CreationFailedError;
use libloading::Library;
use std::ffi::*;
use std::fmt::{Display, Formatter};

pub type Enum = c_uint;
pub type Boolean = c_uint;
pub type Int = c_int;
pub type EGLDisplay = *mut c_void;
pub type EGLSurface = *mut c_void;
pub type NativeDisplayType = *mut c_void;
pub type NativeWindowType = *mut c_void;
pub type EGLConfig = *mut c_void;
pub type EGLContext = *mut c_void;

pub type eglGetError = unsafe extern "system" fn() -> c_int;
pub type eglBindAPI = unsafe extern "system" fn(Enum) -> Boolean;
pub type eglQueryAPI = unsafe extern "system" fn() -> Enum;
pub type eglQueryString = unsafe extern "system" fn(EGLDisplay, Int) -> *const c_char;
pub type eglGetDisplay = unsafe extern "system" fn(NativeDisplayType) -> EGLDisplay;
pub type eglInitialize =
    unsafe extern "system" fn(display: EGLDisplay, major: *mut Int, minor: *mut Int) -> Boolean;
pub type eglTerminate = unsafe extern "system" fn(display: EGLDisplay) -> Boolean;
pub type eglChooseConfig = unsafe extern "system" fn(
    display: EGLDisplay,
    attrib_list: *const Int,
    configs: *mut EGLConfig,
    config_size: Int,
    num_config: *mut Int,
) -> Boolean;

pub type eglGetConfigAttrib = unsafe extern "system" fn(
    display: EGLDisplay,
    config: EGLConfig,
    attribute: Int,
    value: *mut Int,
) -> Boolean;
pub type eglCreateWindowSurface = unsafe extern "system" fn(
    display: EGLDisplay,
    config: EGLConfig,
    win: NativeWindowType,
    attrib_list: *const Int,
) -> EGLSurface;
pub type eglCreateContext = unsafe extern "system" fn(
    display: EGLDisplay,
    config: EGLConfig,
    share_context: EGLContext,
    attrib_list: *const Int,
) -> EGLContext;
pub type eglDestroySurface =
    unsafe extern "system" fn(display: EGLDisplay, surface: EGLSurface) -> Boolean;
pub type eglDestroyContext =
    unsafe extern "system" fn(display: EGLDisplay, context: EGLContext) -> Boolean;
pub type eglMakeCurrent = unsafe extern "system" fn(
    display: EGLDisplay,
    draw: EGLSurface,
    read: EGLSurface,
    ctx: EGLContext,
) -> Boolean;
pub type eglGetProcAddress = unsafe extern "system" fn(procname: *const c_char) -> *const c_void;
pub type eglGetCurrentContext = unsafe extern "system" fn() -> EGLContext;
pub type eglSwapBuffers =
    unsafe extern "system" fn(display: EGLDisplay, surface: EGLSurface) -> Boolean;

pub const ENUM_NONE: Enum = 0x3038;
pub const OPENGL_API: Enum = 0x30A2;
pub const FALSE: Boolean = 0;
pub const NO_DISPLAY: EGLDisplay = 0 as EGLDisplay;
pub const EXTENSIONS: Int = 0x3055;
pub const EGL_SUCCESS: Int = 0x3000;

pub const EGL_SURFACE_TYPE: Int = 0x3033;
pub const EGL_WINDOW_BIT: Int = 0x0004;

pub const EGL_NONE: Int = 0x3038;
pub const EGL_BUFFER_SIZE: Int = 0x3020;
pub const EGL_RED_SIZE: Int = 0x3024;
pub const EGL_GREEN_SIZE: Int = 0x3023;
pub const EGL_BLUE_SIZE: Int = 0x3022;
pub const EGL_ALPHA_SIZE: Int = 0x3021;
pub const EGL_DEPTH_SIZE: Int = 0x3025;
pub const EGL_STENCIL_SIZE: Int = 0x3026;

pub const EGL_GL_COLORSPACE: Int = 0x309D;
pub const EGL_GL_COLORSPACE_LINEAR: Int = 0x308A;
pub const EGL_GL_COLORSPACE_SRGB: Int = 0x3089;

pub const EGL_CONTEXT_MAJOR_VERSION: Int = 0x3098;
pub const EGL_CONTEXT_MINOR_VERSION: Int = 0x30FB;
pub const EGL_CONTEXT_OPENGL_PROFILE_MASK: Int = 0x30FD;
pub const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: Int = 0x00000001;
pub const EGL_CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT: Int = 0x00000002;

pub const EGL_NATIVE_VISUAL_ID: Int = 0x302E;

#[derive(Debug, Copy, Clone)]
pub struct MissingSymbolError {
    name: &'static CStr,
}

impl Display for MissingSymbolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Missing EGL symbol: {}", self.name.to_string_lossy())
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
    pub eglGetDisplay: eglGetDisplay,
    pub eglInitialize: eglInitialize,
    pub eglTerminate: eglTerminate,
    pub eglChooseConfig: eglChooseConfig,
    pub eglGetConfigAttrib: eglGetConfigAttrib,
    pub eglCreateWindowSurface: eglCreateWindowSurface,
    pub eglDestroySurface: eglDestroySurface,
    pub eglCreateContext: eglCreateContext,
    pub eglGetProcAddress: eglGetProcAddress,
    pub eglMakeCurrent: eglMakeCurrent,
    pub eglDestroyContext: eglDestroyContext,
    pub eglGetCurrentContext: eglGetCurrentContext,
    pub eglSwapBuffers: eglSwapBuffers,
}

impl Functions {
    pub unsafe fn load_from(library: &Library) -> Result<Self, CreationFailedError> {
        Ok(Self {
            eglGetError: Self::get(library, c"eglGetError")?,
            eglBindAPI: Self::get(library, c"eglBindAPI")?,
            eglQueryAPI: Self::get(library, c"eglQueryAPI")?,
            eglQueryString: Self::get(library, c"eglQueryString")?,
            eglGetDisplay: Self::get(library, c"eglGetDisplay")?,
            eglInitialize: Self::get(library, c"eglInitialize")?,
            eglTerminate: Self::get(library, c"eglTerminate")?,
            eglChooseConfig: Self::get(library, c"eglChooseConfig")?,
            eglGetConfigAttrib: Self::get(library, c"eglGetConfigAttrib")?,
            eglCreateWindowSurface: Self::get(library, c"eglCreateWindowSurface")?,
            eglDestroySurface: Self::get(library, c"eglDestroySurface")?,
            eglCreateContext: Self::get(library, c"eglCreateContext")?,
            eglMakeCurrent: Self::get(library, c"eglMakeCurrent")?,
            eglGetProcAddress: Self::get(library, c"eglGetProcAddress")?,
            eglDestroyContext: Self::get(library, c"eglDestroyContext")?,
            eglGetCurrentContext: Self::get(library, c"eglGetCurrentContext")?,
            eglSwapBuffers: Self::get(library, c"eglSwapBuffers")?,
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
