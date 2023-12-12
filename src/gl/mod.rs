use std::ffi::c_void;
use std::marker::PhantomData;

// On X11 creating the context is a two step process
#[cfg(not(target_os = "linux"))]
use raw_window_handle::RawWindowHandle;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win as platform;

// We need to use this directly within the X11 window creation to negotiate the correct visual
#[cfg(target_os = "linux")]
pub(crate) mod x11;
#[cfg(target_os = "linux")]
pub(crate) use self::x11 as platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[derive(Clone, Debug)]
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
    CreationFailed(platform::CreationFailedError),
}

pub struct GlContext {
    context: platform::GlContext,
    phantom: PhantomData<*mut ()>,
}

impl GlContext {
    #[cfg(not(target_os = "linux"))]
    pub(crate) unsafe fn create(
        parent: &RawWindowHandle, config: GlConfig,
    ) -> Result<GlContext, GlError> {
        platform::GlContext::create(parent, config)
            .map(|context| GlContext { context, phantom: PhantomData })
    }

    /// The X11 version needs to be set up in a different way compared to the Windows and macOS
    /// versions. So the platform-specific versions should be used to construct the context within
    /// baseview, and then this object can be passed to the user.
    #[cfg(target_os = "linux")]
    pub(crate) fn new(context: platform::GlContext) -> GlContext {
        GlContext { context, phantom: PhantomData }
    }

    pub unsafe fn make_current(&self) {
        self.context.make_current();
    }

    pub unsafe fn make_not_current(&self) {
        self.context.make_not_current();
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        self.context.get_proc_address(symbol)
    }

    pub fn swap_buffers(&self) {
        self.context.swap_buffers();
    }

    /// On macOS the `NSOpenGLView` needs to be resized separtely from our main view.
    #[cfg(target_os = "macos")]
    pub(crate) fn resize(&self, size: cocoa::foundation::NSSize) {
        self.context.resize(size);
    }
}
