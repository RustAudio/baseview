use crate::gl::{GlConfig, Profile};
use crate::wrappers::win32::window::OwnDeviceContext;
use std::ffi::{c_void, CStr};
use std::fmt::{Display, Formatter};
use std::mem::transmute;
use std::num::NonZeroI32;
use std::ptr::{null_mut, NonNull};
use windows_core::Error;
use windows_sys::Win32::Graphics::Gdi::HDC;
use windows_sys::Win32::Graphics::OpenGL::{
    wglDeleteContext, wglGetCurrentContext, wglGetProcAddress, wglMakeCurrent, HGLRC,
};

const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;
const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: i32 = 0x00000001;
const WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: i32 = 0x00000002;

pub struct WglContext {
    pub(super) inner: NonNull<c_void>,
}

impl WglContext {
    pub unsafe fn make_current(&self, dc: &OwnDeviceContext) -> windows_core::Result<()> {
        let result = unsafe { wglMakeCurrent(dc.as_raw(), self.inner.as_ptr()) };
        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }

    pub unsafe fn make_not_current(&self) -> windows_core::Result<()> {
        let current = unsafe { wglGetCurrentContext() };

        if current.is_null() {
            return Ok(());
        };

        if current != self.inner.as_ptr() {
            return Ok(());
        };

        let result = unsafe { wglMakeCurrent(null_mut(), null_mut()) };
        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }

    pub fn with_current<T>(
        &self, dc: &OwnDeviceContext, f: impl FnOnce() -> T,
    ) -> windows_core::Result<T> {
        struct Guard<'a>(&'a WglContext);
        impl<'a> Drop for Guard<'a> {
            fn drop(&mut self) {
                let _ = unsafe { self.0.make_not_current() };
            }
        }

        unsafe { self.make_current(dc)? };

        let _guard = Guard(self);

        let result = f();

        drop(_guard);

        Ok(result)
    }
}

impl Drop for WglContext {
    fn drop(&mut self) {
        let _ = unsafe { self.make_not_current() };
        unsafe { wglDeleteContext(self.inner.as_ptr()) }; // TODO: warn on error
    }
}

// See https://www.khronos.org/registry/OpenGL/extensions/EXT/WGL_EXT_swap_control.txt
type WglSwapIntervalEXT = unsafe extern "system" fn(i32) -> i32;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/WGL_ARB_pixel_format.txt
type WglChoosePixelFormatARB =
    unsafe extern "system" fn(HDC, *const i32, *const f32, u32, *mut i32, *mut u32) -> i32;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/WGL_ARB_create_context.txt
type WglCreateContextAttribsARB = unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;

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

    pub fn create_context_for_config(
        &self, dc: &OwnDeviceContext, config: &GlConfig,
    ) -> windows_core::Result<WglContext> {
        let profile_mask = match config.profile {
            Profile::Core => WGL_CONTEXT_CORE_PROFILE_BIT_ARB,
            Profile::Compatibility => WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB,
        };

        #[rustfmt::skip]
        let ctx_attribs = [
            WGL_CONTEXT_MAJOR_VERSION_ARB, config.version.0 as i32,
            WGL_CONTEXT_MINOR_VERSION_ARB, config.version.1 as i32,
            WGL_CONTEXT_PROFILE_MASK_ARB, profile_mask,
            0
        ];

        let ctx = unsafe {
            (self.wglCreateContextAttribsARB)(dc.as_raw(), null_mut(), ctx_attribs.as_ptr())
        };

        let ctx = NonNull::new(ctx).ok_or_else(Error::from_thread)?;

        Ok(WglContext { inner: ctx })
    }

    pub fn set_vsync(&self, vsync: bool) -> windows_core::Result<()> {
        let result = unsafe { (self.wglSwapIntervalEXT)(vsync.into()) };
        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }

    pub fn choose_pixel_format_from_attribs(
        &self, attribs: &PixelFormatAttribs, dc: &OwnDeviceContext,
    ) -> windows_core::Result<Option<NonZeroI32>> {
        let mut pixel_formats = [0];
        let mut num_formats = 0;
        let result = unsafe {
            (self.wglChoosePixelFormatARB)(
                dc.as_raw(),
                attribs.inner.as_ptr(),
                std::ptr::null(),
                1,
                pixel_formats.as_mut_ptr(),
                &mut num_formats,
            )
        };

        if result == 0 {
            return Err(Error::from_thread());
        }

        if num_formats == 0 {
            return Ok(None);
        }

        Ok(NonZeroI32::new(pixel_formats[0]))
    }
}

const WGL_DRAW_TO_WINDOW_ARB: i32 = 0x2001;
const WGL_ACCELERATION_ARB: i32 = 0x2003;
const WGL_SUPPORT_OPENGL_ARB: i32 = 0x2010;
const WGL_DOUBLE_BUFFER_ARB: i32 = 0x2011;
const WGL_PIXEL_TYPE_ARB: i32 = 0x2013;
const WGL_RED_BITS_ARB: i32 = 0x2015;
const WGL_GREEN_BITS_ARB: i32 = 0x2017;
const WGL_BLUE_BITS_ARB: i32 = 0x2019;
const WGL_ALPHA_BITS_ARB: i32 = 0x201B;
const WGL_DEPTH_BITS_ARB: i32 = 0x2022;
const WGL_STENCIL_BITS_ARB: i32 = 0x2023;

const WGL_FULL_ACCELERATION_ARB: i32 = 0x2027;
const WGL_TYPE_RGBA_ARB: i32 = 0x202B;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/ARB_multisample.txt

const WGL_SAMPLE_BUFFERS_ARB: i32 = 0x2041;
const WGL_SAMPLES_ARB: i32 = 0x2042;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/ARB_framebuffer_sRGB.txt

const WGL_FRAMEBUFFER_SRGB_CAPABLE_ARB: i32 = 0x20A9;

pub struct PixelFormatAttribs {
    inner: [i32; 29],
}

impl PixelFormatAttribs {
    #[rustfmt::skip]
    pub fn from_config(config: &GlConfig) -> Self {
        Self {
            inner: [
                WGL_DRAW_TO_WINDOW_ARB, 1,
                WGL_ACCELERATION_ARB, WGL_FULL_ACCELERATION_ARB,
                WGL_SUPPORT_OPENGL_ARB, 1,
                WGL_DOUBLE_BUFFER_ARB, config.double_buffer as i32,
                WGL_PIXEL_TYPE_ARB, WGL_TYPE_RGBA_ARB,
                WGL_RED_BITS_ARB, config.red_bits as i32,
                WGL_GREEN_BITS_ARB, config.green_bits as i32,
                WGL_BLUE_BITS_ARB, config.blue_bits as i32,
                WGL_ALPHA_BITS_ARB, config.alpha_bits as i32,
                WGL_DEPTH_BITS_ARB, config.depth_bits as i32,
                WGL_STENCIL_BITS_ARB, config.stencil_bits as i32,
                WGL_SAMPLE_BUFFERS_ARB, config.samples.is_some() as i32,
                WGL_SAMPLES_ARB, config.samples.unwrap_or(0) as i32,
                WGL_FRAMEBUFFER_SRGB_CAPABLE_ARB, config.srgb as i32,
                0,
            ]
        }
    }

    pub fn set_without_srgb_ext(&mut self) {
        self.inner[26] = 0;
        self.inner[27] = 0;
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
