use std::ffi::{c_void, CString};
use std::rc::Rc;
use windows_sys::{
    core::s,
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        Graphics::OpenGL::{wglGetProcAddress, wglMakeCurrent},
        System::LibraryLoader::{GetProcAddress, LoadLibraryA},
    },
};

use crate::gl::*;
use crate::wrappers::win32::window::{
    with_dummy_window, HWnd, MissingExtensionFunctionError, OwnDeviceContext, PixelFormat,
    WglContext, WglExtra,
};

const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;

const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: i32 = 0x00000001;
const WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: i32 = 0x00000002;

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

#[derive(Debug)]
pub enum CreationFailedError {
    Win32(windows_core::Error),
    MissingWglExtension(MissingExtensionFunctionError),
}

impl From<windows_core::Error> for CreationFailedError {
    fn from(err: windows_core::Error) -> Self {
        CreationFailedError::Win32(err)
    }
}

pub type GlContext = Rc<GlContextInner>;

impl From<CreationFailedError> for GlError {
    fn from(e: CreationFailedError) -> Self {
        GlError::CreationFailed(e)
    }
}

pub struct GlContextInner {
    hwnd: HWnd,
    hdc: OwnDeviceContext,
    hglrc: WglContext,
    gl_library: HMODULE,
}

impl GlContextInner {
    pub unsafe fn create(window: HWnd, config: GlConfig) -> Result<Self, GlError> {
        // Create temporary window and context to load function pointers
        let extra = with_dummy_window(|hwnd_tmp| {
            let hdc = hwnd_tmp.get_own_dc()?;
            hdc.set_pixel_format(&PixelFormat::default())?;

            let wgl_ctx = hdc.create_wgl_context()?;
            wgl_ctx.with_current(&hdc, || WglExtra::load())
        })??;

        // Create actual context

        let hwnd = window.as_raw();

        let hdc = window.get_own_dc()?;

        // Try to choose pixel format with requested config
        #[rustfmt::skip]
        let pixel_format_attribs = [
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
        ];

        let mut pixel_format = 0;
        let mut num_formats = 0;
        extra.wglChoosePixelFormatARB.unwrap()(
            hdc.as_raw(),
            pixel_format_attribs.as_ptr(),
            std::ptr::null(),
            1,
            &mut pixel_format,
            &mut num_formats,
        );

        // If no matching format found and sRGB was requested, try again without sRGB
        if num_formats == 0 && config.srgb {
            eprintln!("Warning: sRGB framebuffer not supported, falling back to non-sRGB");

            #[rustfmt::skip]
            let pixel_format_attribs_fallback = [
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
                // WGL_FRAMEBUFFER_SRGB_CAPABLE_ARB omitted
                0,
            ];

            extra.wglChoosePixelFormatARB.unwrap()(
                hdc.as_raw(),
                pixel_format_attribs_fallback.as_ptr(),
                std::ptr::null(),
                1,
                &mut pixel_format,
                &mut num_formats,
            );
        }

        // if no num_formats are found which happens in Wine for child windows, use fallback
        if num_formats == 0 {
            hdc.set_pixel_format(&PixelFormat::from_config(&config))?;
        }

        hdc.set_pixel_format_from_index(pixel_format)?;

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

        let hglrc = extra.wglCreateContextAttribsARB.unwrap()(
            hdc,
            std::ptr::null_mut(),
            ctx_attribs.as_ptr(),
        );
        if hglrc.is_null() {
            return Err(GlError::CreationFailed(()));
        }

        let gl_library = LoadLibraryA(s!("opengl32.dll"));

        wglMakeCurrent(hdc, hglrc);
        extra.wglSwapIntervalEXT.unwrap()(config.vsync as i32);
        wglMakeCurrent(hdc, std::ptr::null_mut());

        Ok(Self { hwnd, hdc, hglrc, gl_library })
    }

    pub unsafe fn make_current(&self) {
        let _ = self.hglrc.make_current(&self.hdc);
    }

    pub unsafe fn make_not_current(&self) {
        let _ = self.hglrc.make_not_current();
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol = CString::new(symbol).unwrap();
        let symbol_ptr = symbol.as_ptr().cast();

        let addr = unsafe {
            wglGetProcAddress(symbol_ptr).or_else(|| GetProcAddress(self.gl_library, symbol_ptr))
        };

        match addr {
            Some(addr) => addr as *const c_void,
            None => std::ptr::null(),
        }
    }

    pub fn swap_buffers(&self) {
        let _ = self.hdc.swap_buffers();
    }
}

impl Drop for GlContextInner {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.gl_library);
        }
    }
}
