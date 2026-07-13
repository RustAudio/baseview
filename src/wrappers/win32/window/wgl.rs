use std::ffi::CStr;
use std::fmt::{Display, Formatter};
use std::mem::transmute;
use windows_sys::Win32::Graphics::Gdi::HDC;
use windows_sys::Win32::Graphics::OpenGL::{wglGetProcAddress, HGLRC};

// See https://www.khronos.org/registry/OpenGL/extensions/EXT/WGL_EXT_swap_control.txt
type WglSwapIntervalEXT = extern "system" fn(i32) -> i32;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/WGL_ARB_pixel_format.txt
type WglChoosePixelFormatARB =
    extern "system" fn(HDC, *const i32, *const f32, u32, *mut i32, *mut u32) -> i32;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/WGL_ARB_create_context.txt
type WglCreateContextAttribsARB = extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;

#[allow(non_snake_case)]
pub struct WglExtra {
    wglCreateContextAttribsARB: WglCreateContextAttribsARB,
    wglChoosePixelFormatARB: WglChoosePixelFormatARB,
    wglSwapIntervalEXT: WglSwapIntervalEXT,
}

impl WglExtra {
    pub fn load() -> Result<Self, MissingExtensionFunctionError> {
        unsafe {
            Ok(Self {
                wglCreateContextAttribsARB: transmute(Self::load_fn(
                    c"wglCreateContextAttribsARB",
                )?),
                wglChoosePixelFormatARB: transmute(Self::load_fn(c"wglChoosePixelFormatARB")?),
                wglSwapIntervalEXT: transmute(Self::load_fn(c"wglSwapIntervalEXT")?),
            })
        }
    }

    fn load_fn(
        name: &'static CStr,
    ) -> Result<unsafe extern "system" fn() -> isize, MissingExtensionFunctionError> {
        unsafe { wglGetProcAddress(name.as_ptr() as *const u8) }
            .ok_or_else(|| MissingExtensionFunctionError { name })
    }
}

#[derive(Debug)]
pub struct MissingExtensionFunctionError {
    name: &'static CStr,
}

impl Display for MissingExtensionFunctionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Missing WGL function extension: {}", self.name.to_string_lossy())
    }
}
