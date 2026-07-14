use std::ffi::{c_void, CString};
use std::num::NonZeroI32;
use std::rc::Rc;
use windows_core::{s, PCSTR};
use windows_sys::Win32::Graphics::OpenGL::wglGetProcAddress;

use crate::gl::*;
use crate::wrappers::win32::window::{
    with_dummy_window, HWnd, MissingExtensionFunctionError, OwnDeviceContext, PixelFormat,
    PixelFormatAttribs, WglContext, WglExtra,
};
use crate::wrappers::win32::LibraryModule;

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

impl From<MissingExtensionFunctionError> for CreationFailedError {
    fn from(err: MissingExtensionFunctionError) -> Self {
        CreationFailedError::MissingWglExtension(err)
    }
}

pub type GlContext = Rc<GlContextInner>;

impl From<CreationFailedError> for GlError {
    fn from(e: CreationFailedError) -> Self {
        GlError::CreationFailed(e)
    }
}

pub struct GlContextInner {
    hdc: OwnDeviceContext,
    wgl_ctx: WglContext,
    gl_library: LibraryModule,
}

impl GlContextInner {
    pub unsafe fn create(window: HWnd, config: GlConfig) -> Result<Self, CreationFailedError> {
        let gl_library = LibraryModule::load(s!("opengl32.dll"))?;

        // Create temporary window and context to load function pointers
        let extra = with_dummy_window(|hwnd_tmp| {
            let hdc = hwnd_tmp.get_own_dc()?;
            hdc.set_pixel_format(&PixelFormat::default())?;

            let wgl_ctx = hdc.create_wgl_context()?;
            wgl_ctx.with_current(&hdc, WglExtra::load)
        })??;

        // Create actual context
        let hdc = window.get_own_dc()?;
        match find_wgl_pixel_format(&extra, &hdc, &config) {
            Some(format) => hdc.set_pixel_format_from_index(format)?,
            // if no formats are found, which happens in Wine for child windows, use fallback
            None => hdc.set_pixel_format(&PixelFormat::from_config(&config))?,
        }

        let wgl_ctx = extra.create_context_for_config(&hdc, &config)?;

        wgl_ctx.with_current(&hdc, || extra.set_vsync(config.vsync))??;

        Ok(Self { hdc, wgl_ctx, gl_library })
    }

    pub unsafe fn make_current(&self) {
        let _ = self.wgl_ctx.make_current(&self.hdc);
    }

    pub unsafe fn make_not_current(&self) {
        let _ = self.wgl_ctx.make_not_current();
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol = CString::new(symbol).unwrap();
        let symbol_ptr = symbol.as_ptr().cast();

        if let Some(addr) = unsafe { wglGetProcAddress(symbol_ptr) } {
            return addr as *const c_void;
        }

        let symbol_ptr = PCSTR::from_raw(symbol_ptr);
        if let Some(addr) = unsafe { self.gl_library.get_proc_address(symbol_ptr) } {
            return addr;
        }

        core::ptr::null()
    }

    pub fn swap_buffers(&self) {
        let _ = self.hdc.swap_buffers();
    }
}

fn find_wgl_pixel_format(
    extra: &WglExtra, dc: &OwnDeviceContext, config: &GlConfig,
) -> Option<NonZeroI32> {
    let mut format_attribs = PixelFormatAttribs::from_config(config);

    // TODO: log errors
    if let Ok(Some(format)) = extra.choose_pixel_format_from_attribs(&format_attribs, dc) {
        return Some(format);
    };

    eprintln!("Warning: sRGB framebuffer not supported, falling back to non-sRGB");
    format_attribs.set_without_srgb_ext();

    if let Ok(Some(format)) = extra.choose_pixel_format_from_attribs(&format_attribs, dc) {
        return Some(format);
    };

    None
}
