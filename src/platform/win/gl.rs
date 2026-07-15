use std::ffi::{c_void, CStr};
use std::num::NonZeroI32;
use std::rc::Rc;
use windows_core::{s, PCSTR};
use windows_sys::Win32::Graphics::OpenGL::wglGetProcAddress;

use crate::gl::*;
use crate::warn;
use crate::wrappers::win32::window::{
    with_dummy_window, HWnd, OwnDeviceContext, PixelFormat, PixelFormatAttribs, WglContext,
    WglExtra,
};
use crate::wrappers::win32::LibraryModule;

pub type GlContext = Rc<GlContextInner>;

pub struct GlContextInner {
    hdc: OwnDeviceContext,
    wgl_ctx: WglContext,
    gl_library: LibraryModule,
}

impl GlContextInner {
    pub fn create(window: HWnd, config: GlConfig) -> Result<Self, windows_core::Error> {
        let gl_library = unsafe { LibraryModule::load(s!("opengl32.dll"))? };

        // Create temporary window and context to load function pointers
        let extra = with_dummy_window(|hwnd_tmp| {
            let hdc = hwnd_tmp.get_own_dc()?;
            hdc.set_pixel_format(&PixelFormat::default())?;

            let wgl_ctx = hdc.create_wgl_context()?;
            wgl_ctx.with_current(&hdc, WglExtra::load)
        })?;

        // Create actual context
        let hdc = window.get_own_dc()?;
        match find_wgl_pixel_format(&extra, &hdc, &config) {
            Some(format) => hdc.set_pixel_format_from_index(format)?,
            // if no formats are found, which happens in Wine for child windows, use fallback
            None => hdc.set_pixel_format(&PixelFormat::from_config(&config))?,
        }

        let wgl_ctx = match extra.create_context_for_config(&hdc, &config) {
            Ok(wgl_ctx) => wgl_ctx,
            Err(e) => {
                warn!("Could not create OpenGL context from OpenGL config attributes: {}. Attempting fallback.", e);
                hdc.create_wgl_context()?
            }
        };

        if let Err(e) | Ok(Err(e)) = wgl_ctx.with_current(&hdc, || extra.set_vsync(config.vsync)) {
            warn!("Could not set vsync: {}", e);
        }

        Ok(Self { hdc, wgl_ctx, gl_library })
    }

    pub unsafe fn make_current(&self) {
        let _ = self.wgl_ctx.make_current(&self.hdc);
    }

    pub unsafe fn make_not_current(&self) {
        let _ = self.wgl_ctx.make_not_current();
    }

    pub fn get_proc_address(&self, symbol: &CStr) -> *const c_void {
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

    match extra.choose_pixel_format_from_attribs(&format_attribs, dc) {
        Ok(Some(format)) => return Some(format),
        Err(e) => {
            warn!("Could not choose optimal pixel format from GL configuration: {}", e);
            return None;
        }
        Ok(None) => {}
    };

    eprintln!("Warning: sRGB framebuffer not supported, falling back to non-sRGB");
    format_attribs.set_without_srgb_ext();

    match extra.choose_pixel_format_from_attribs(&format_attribs, dc) {
        Ok(Some(format)) => return Some(format),
        Err(e) => {
            warn!("Could not choose optimal pixel format from GL configuration: {}", e);
            return None;
        }
        Ok(None) => {}
    };

    None
}
