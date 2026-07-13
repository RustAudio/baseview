use std::ffi::{c_void, CString};
use std::rc::Rc;
use windows_sys::{
    core::s,
    Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND},
        Graphics::{
            Gdi::{GetDC, ReleaseDC, HDC},
            OpenGL::{
                wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent,
                ChoosePixelFormat, DescribePixelFormat, SetPixelFormat, SwapBuffers, HGLRC,
                PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL,
                PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
            },
        },
        System::LibraryLoader::{GetProcAddress, LoadLibraryA},
        UI::WindowsAndMessaging::{DestroyWindow, UnregisterClassW},
    },
};

use crate::gl::*;
use crate::wrappers::win32::window::{with_dummy_window, HWnd, PixelFormat};
// See https://www.khronos.org/registry/OpenGL/extensions/ARB/WGL_ARB_create_context.txt

type WglCreateContextAttribsARB = extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;

const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;

const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: i32 = 0x00000001;
const WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: i32 = 0x00000002;

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/WGL_ARB_pixel_format.txt

type WglChoosePixelFormatARB =
    extern "system" fn(HDC, *const i32, *const f32, u32, *mut i32, *mut u32) -> i32;

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

// See https://www.khronos.org/registry/OpenGL/extensions/EXT/WGL_EXT_swap_control.txt

type WglSwapIntervalEXT = extern "system" fn(i32) -> i32;

pub type CreationFailedError = windows_core::Error;
pub type GlContext = Rc<GlContextInner>;

impl From<CreationFailedError> for GlError {
    fn from(e: CreationFailedError) -> Self {
        GlError::CreationFailed(e)
    }
}

#[allow(non_snake_case)]
struct WglExtra {
    wglCreateContextAttribsARB: Option<WglCreateContextAttribsARB>,
    wglChoosePixelFormatARB: Option<WglChoosePixelFormatARB>,
    wglSwapIntervalEXT: Option<WglSwapIntervalEXT>,
}

impl WglExtra {
    pub fn load() -> Self {
        unsafe {
            Self {
                wglCreateContextAttribsARB: wglGetProcAddress(s!("wglCreateContextAttribsARB"))
                    .map(|addr| std::mem::transmute(addr)),
                wglChoosePixelFormatARB: wglGetProcAddress(s!("wglChoosePixelFormatARB"))
                    .map(|addr| std::mem::transmute(addr)),
                wglSwapIntervalEXT: wglGetProcAddress(s!("wglSwapIntervalEXT"))
                    .map(|addr| std::mem::transmute(addr)),
            }
        }
    }
}

pub struct GlContextInner {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
    gl_library: HMODULE,
}

impl GlContextInner {
    pub unsafe fn create(window: HWnd, config: GlConfig) -> Result<Self, GlError> {
        // Create temporary window and context to load function pointers

        let extra = with_dummy_window(|hwnd_tmp| {
            let hdc = hwnd_tmp.get_own_dc()?;
            hdc.set_pixel_format(&PixelFormat::default())?;

            let wgl_ctx = hdc.create_wgl_context()?;
            wgl_ctx.make_current(&hdc)?;

            Ok(WglExtra::load())
        })?;

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
            hdc,
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
                hdc,
                pixel_format_attribs_fallback.as_ptr(),
                std::ptr::null(),
                1,
                &mut pixel_format,
                &mut num_formats,
            );
        }

        // if no num_formats are found which happens in Wine for child windows, use fallback
        if num_formats == 0 {
            let fallback_pfd = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                cAlphaBits: config.alpha_bits,
                cDepthBits: config.depth_bits,
                cStencilBits: config.stencil_bits,
                iLayerType: PFD_MAIN_PLANE as u8,
                ..std::mem::zeroed()
            };
            pixel_format = ChoosePixelFormat(hdc, &fallback_pfd);
        }

        if pixel_format == 0 {
            ReleaseDC(hwnd, hdc);
            return Err(GlError::CreationFailed(()));
        }

        hdc.set_pixel_format_from_index(pixel_format)?;

        let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
        DescribePixelFormat(
            hdc,
            pixel_format,
            std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u32,
            &mut pfd,
        );
        SetPixelFormat(hdc, pixel_format, &pfd);

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
        wglMakeCurrent(self.hdc, self.hglrc);
    }

    pub unsafe fn make_not_current(&self) {
        wglMakeCurrent(self.hdc, std::ptr::null_mut());
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
        unsafe {
            SwapBuffers(self.hdc);
        }
    }
}

impl Drop for GlContextInner {
    fn drop(&mut self) {
        unsafe {
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            wglDeleteContext(self.hglrc);
            ReleaseDC(self.hwnd, self.hdc);
            FreeLibrary(self.gl_library);
        }
    }
}
