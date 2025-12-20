use std::ffi::{c_void, CString, OsStr};
use std::os::windows::ffi::OsStrExt;

use windows::{
    core::{s, PCSTR, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Gdi::{GetDC, ReleaseDC, HDC},
            OpenGL::{
                wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent,
                ChoosePixelFormat, DescribePixelFormat, SetPixelFormat, SwapBuffers, HGLRC,
                PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL,
                PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
            },
        },
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryA},
            SystemServices::IMAGE_DOS_HEADER,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, UnregisterClassW,
            CS_OWNDC, CW_USEDEFAULT, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
        },
    },
};

use raw_window_handle::RawWindowHandle;

use super::{GlConfig, GlError, Profile};

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

pub type CreationFailedError = ();
pub struct GlContext {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
    gl_library: HMODULE,
}

extern "C" {
    static __ImageBase: IMAGE_DOS_HEADER;
}

impl GlContext {
    #[allow(unused)]
    pub unsafe fn create(parent: &RawWindowHandle, config: GlConfig) -> Result<GlContext, GlError> {
        let handle = if let RawWindowHandle::Win32(handle) = parent {
            handle
        } else {
            return Err(GlError::InvalidWindowHandle);
        };

        if handle.hwnd.is_null() {
            return Err(GlError::InvalidWindowHandle);
        }

        // Create temporary window and context to load function pointers

        let class_name_str = format!("raw-gl-context-window-{}", uuid::Uuid::new_v4().to_simple());
        let mut class_name: Vec<u16> = OsStr::new(&class_name_str).encode_wide().collect();
        class_name.push(0);

        let pcw_class_name = PCWSTR(class_name.as_ptr());

        let hinstance = HINSTANCE(&__ImageBase as *const IMAGE_DOS_HEADER as *mut c_void);

        extern "system" fn wndproc(
            hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        let wnd_class = WNDCLASSW {
            style: CS_OWNDC,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: pcw_class_name,
            ..std::mem::zeroed()
        };

        let class = RegisterClassW(&wnd_class);
        if class == 0 {
            return Err(GlError::CreationFailed(()));
        }

        let hwnd_tmp = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw_class_name,
            None,
            WINDOW_STYLE(0),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(hinstance),
            None,
        );

        if hwnd_tmp.is_err() {
            return Err(GlError::CreationFailed(()));
        }
        let hwnd_tmp = hwnd_tmp.ok();

        let hdc_tmp = GetDC(hwnd_tmp);

        let pfd_tmp = PIXELFORMATDESCRIPTOR {
            nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 32,
            cAlphaBits: 8,
            cDepthBits: 24,
            cStencilBits: 8,
            iLayerType: PFD_MAIN_PLANE.0 as u8,
            ..std::mem::zeroed()
        };

        SetPixelFormat(hdc_tmp, ChoosePixelFormat(hdc_tmp, &pfd_tmp), &pfd_tmp);

        let hglrc_tmp = wglCreateContext(hdc_tmp);
        if hglrc_tmp.is_err() {
            ReleaseDC(hwnd_tmp, hdc_tmp);
            UnregisterClassW(pcw_class_name, Some(hinstance));
            DestroyWindow(hwnd_tmp.unwrap());
            return Err(GlError::CreationFailed(()));
        }
        let hglrc_tmp = hglrc_tmp.unwrap();

        wglMakeCurrent(hdc_tmp, hglrc_tmp);

        #[allow(non_snake_case)]
        let wglCreateContextAttribsARB: Option<WglCreateContextAttribsARB> = {
            wglGetProcAddress(s!("wglCreateContextAttribsARB"))
                .map(|addr| std::mem::transmute(addr))
        };

        #[allow(non_snake_case)]
        let wglChoosePixelFormatARB: Option<WglChoosePixelFormatARB> = {
            wglGetProcAddress(s!("wglChoosePixelFormatARB")).map(|addr| std::mem::transmute(addr))
        };

        #[allow(non_snake_case)]
        let wglSwapIntervalEXT: Option<WglSwapIntervalEXT> =
            { wglGetProcAddress(s!("wglSwapIntervalEXT")).map(|addr| std::mem::transmute(addr)) };

        wglMakeCurrent(hdc_tmp, HGLRC::default());
        wglDeleteContext(hglrc_tmp);
        ReleaseDC(hwnd_tmp, hdc_tmp);
        UnregisterClassW(wnd_class.lpszClassName, Some(hinstance));
        DestroyWindow(hwnd_tmp.unwrap());

        // Create actual context

        let hwnd = HWND(handle.hwnd);

        let hdc = GetDC(Some(hwnd));

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
        wglChoosePixelFormatARB.unwrap()(
            hdc,
            pixel_format_attribs.as_ptr(),
            std::ptr::null(),
            1,
            &mut pixel_format,
            &mut num_formats,
        );

        let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
        DescribePixelFormat(
            hdc,
            pixel_format,
            std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u32,
            Some(&mut pfd),
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

        let hglrc =
            wglCreateContextAttribsARB.unwrap()(hdc, HGLRC::default(), ctx_attribs.as_ptr());
        if hglrc.is_invalid() {
            return Err(GlError::CreationFailed(()));
        }

        let gl_library = LoadLibraryA(s!("opengl32.dll")).unwrap_or_default();

        wglMakeCurrent(hdc, hglrc);
        wglSwapIntervalEXT.unwrap()(config.vsync as i32);
        wglMakeCurrent(hdc, HGLRC::default());

        Ok(GlContext { hwnd, hdc, hglrc, gl_library })
    }

    #[allow(unused)]
    pub unsafe fn make_current(&self) {
        wglMakeCurrent(self.hdc, self.hglrc);
    }

    #[allow(unused)]
    pub unsafe fn make_not_current(&self) {
        wglMakeCurrent(self.hdc, HGLRC::default());
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol = CString::new(symbol).unwrap();
        unsafe {
            wglGetProcAddress(PCSTR(symbol.as_ptr() as *const u8))
                .map(|p| p as *const c_void)
                .or_else(|| {
                    GetProcAddress(self.gl_library, PCSTR(symbol.as_ptr() as *const u8))
                        .map(|p| p as *const c_void)
                })
                .unwrap_or_default()
        }
    }

    pub fn swap_buffers(&self) {
        #[allow(unused)]
        unsafe {
            SwapBuffers(self.hdc);
        }
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        #[allow(unused)]
        unsafe {
            wglMakeCurrent(HDC::default(), HGLRC::default());
            wglDeleteContext(self.hglrc);
            ReleaseDC(Some(self.hwnd), self.hdc);
            FreeLibrary(self.gl_library);
        }
    }
}
