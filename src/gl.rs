use crate::platform::gl::*;
use std::ffi::c_void;
use std::marker::PhantomData;

#[derive(Clone, Debug, PartialEq)]
pub struct GlConfig {
    pub version: (u8, u8),
    pub profile: Profile,
    pub red_bits: u8,
    pub blue_bits: u8,
    pub green_bits: u8,
    pub alpha_bits: u8,
    pub depth_bits: u8,
    pub stencil_bits: u8,
    pub samples: Option<u8>,
    pub srgb: bool,
    pub double_buffer: bool,
    pub vsync: bool,
}

impl Default for GlConfig {
    fn default() -> Self {
        GlConfig {
            version: (3, 2),
            profile: Profile::Core,
            red_bits: 8,
            blue_bits: 8,
            green_bits: 8,
            alpha_bits: 8,
            depth_bits: 24,
            stencil_bits: 8,
            samples: None,
            srgb: true,
            double_buffer: true,
            vsync: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Compatibility,
    Core,
}

#[derive(Debug)]
pub enum GlError {
    InvalidWindowHandle,
    VersionNotSupported,
    CreationFailed(CreationFailedError),
}

#[derive(Clone)]
pub struct GlContext {
    inner: crate::platform::gl::GlContext,
    // To make sure this is !Send, !Sync, and !UnwindSafe on all platforms
    phantom: PhantomData<(*mut (), &'static mut ())>,
}

impl GlContext {
    pub(crate) fn new(context: crate::platform::gl::GlContext) -> GlContext {
        GlContext { inner: context, phantom: PhantomData }
    }

    pub unsafe fn make_current(&self) {
        self.inner.make_current();
    }

    pub unsafe fn make_not_current(&self) {
        self.inner.make_not_current();
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        self.inner.get_proc_address(symbol)
    }

    pub fn swap_buffers(&self) {
        self.inner.swap_buffers();
    }
}
