use crate::gl::GlConfig;
use crate::wrappers::win32::window::HWnd;
use std::ffi::c_void;
use std::num::{NonZero, NonZeroI32};
use std::ptr::{null_mut, NonNull};
use windows_core::{Error, Result};
use windows_sys::Win32::Graphics::Gdi::{GetDC, HDC};
use windows_sys::Win32::Graphics::OpenGL::{
    wglCreateContext, wglDeleteContext, wglGetCurrentContext, wglGetProcAddress, wglMakeCurrent,
    ChoosePixelFormat, DescribePixelFormat, SetPixelFormat, SwapBuffers, PFD_DOUBLEBUFFER,
    PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
};

pub struct OwnDeviceContext {
    inner: NonNull<c_void>,
}

impl OwnDeviceContext {
    pub(super) fn from_window(window: HWnd) -> Result<Self> {
        let dc = unsafe { GetDC(window.as_raw()) };

        let dc = NonNull::new(dc).ok_or_else(Error::from_thread)?;

        Ok(Self { inner: dc })
    }

    pub fn as_raw(&self) -> HDC {
        self.inner.as_ptr()
    }

    fn describe_pixel_format(&self, index: NonZeroI32) -> Result<PIXELFORMATDESCRIPTOR> {
        let mut desc = PIXELFORMATDESCRIPTOR {
            nSize: size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            ..Default::default()
        };

        let result = unsafe {
            DescribePixelFormat(
                self.as_raw(),
                index.get(),
                size_of::<PIXELFORMATDESCRIPTOR>() as u32,
                &mut desc,
            )
        };

        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(desc)
    }

    pub fn set_pixel_format(&self, pixel_format: &PixelFormat) -> Result<()> {
        let desc = pixel_format.to_raw_descriptor();
        let index = unsafe { ChoosePixelFormat(self.as_raw(), &desc) };
        let Some(index) = NonZero::new(index) else { return Err(Error::from_thread()) };

        let result = unsafe { SetPixelFormat(self.as_raw(), index.get(), &desc) };

        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }

    pub fn set_pixel_format_from_index(&self, index: NonZeroI32) -> Result<()> {
        let desc = self.describe_pixel_format(index)?;
        let result = unsafe { SetPixelFormat(self.as_raw(), index.get(), &desc) };

        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }

    pub fn create_wgl_context(&self) -> Result<WglContext> {
        let ctx = unsafe { wglCreateContext(self.as_raw()) };
        let ctx = NonNull::new(ctx).ok_or_else(Error::from_thread)?;

        Ok(WglContext { inner: ctx })
    }

    pub fn swap_buffers(&self) -> Result<()> {
        let result = unsafe { SwapBuffers(self.as_raw()) };
        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }
}

pub struct WglContext {
    inner: NonNull<c_void>,
}

impl WglContext {
    pub unsafe fn make_current(&self, dc: &OwnDeviceContext) -> Result<()> {
        let result = unsafe { wglMakeCurrent(dc.as_raw(), self.inner.as_ptr()) };
        if result == 0 {
            return Err(Error::from_thread());
        }

        Ok(())
    }

    pub unsafe fn make_not_current(&self) -> Result<()> {
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

    pub fn with_current<T>(&self, dc: &OwnDeviceContext, f: impl FnOnce() -> T) -> Result<T> {
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

#[derive(Copy, Clone)]
pub struct PixelFormat {
    pub alpha_bits: u8,
    pub depth_bits: u8,
    pub stencil_bits: u8,
}

impl Default for PixelFormat {
    fn default() -> Self {
        Self { alpha_bits: 8, depth_bits: 24, stencil_bits: 8 }
    }
}

impl PixelFormat {
    pub fn from_config(config: &GlConfig) -> Self {
        Self {
            alpha_bits: config.alpha_bits,
            depth_bits: config.depth_bits,
            stencil_bits: config.stencil_bits,
        }
    }

    pub fn to_raw_descriptor(&self) -> PIXELFORMATDESCRIPTOR {
        PIXELFORMATDESCRIPTOR {
            nSize: size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 32,
            cAlphaBits: self.alpha_bits,
            cDepthBits: self.depth_bits,
            cStencilBits: self.stencil_bits,
            iLayerType: PFD_MAIN_PLANE as u8,
            ..Default::default()
        }
    }
}
