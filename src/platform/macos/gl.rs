#![allow(deprecated)] // OpenGL is deprecated on macOS

use crate::gl::{GlConfig, Profile};
use crate::platform::*;
use crate::warn;
use objc2::rc::Retained;
use objc2::AllocAnyThread;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSOpenGLContext, NSOpenGLContextParameter, NSOpenGLPFAAccelerated, NSOpenGLPFAAlphaSize,
    NSOpenGLPFAColorSize, NSOpenGLPFADepthSize, NSOpenGLPFADoubleBuffer, NSOpenGLPFAMultisample,
    NSOpenGLPFAOpenGLProfile, NSOpenGLPFASampleBuffers, NSOpenGLPFASamples, NSOpenGLPFAStencilSize,
    NSOpenGLPixelFormat, NSOpenGLProfileVersion3_2Core, NSOpenGLProfileVersion4_1Core,
    NSOpenGLProfileVersionLegacy, NSOpenGLView, NSView,
};
use objc2_core_foundation::{CFBundle, CFRetained, CFString, CFStringBuiltInEncodings};
use objc2_foundation::NSSize;
use std::ffi::{c_void, CStr};
use std::fmt::Display;
use std::ptr::NonNull;

#[derive(Debug)]
pub enum GlError {
    GlVersionNotSupported { version: (u8, u8), profile: Profile },
    NSOpenGLPixelFormatInitFailed,
    NSOpenGLViewInitFailed,
    OpenGlBundleNotFound,
}

impl From<GlError> for Error {
    fn from(value: GlError) -> Self {
        Self::GlError(value)
    }
}

impl Display for GlError {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GlError::GlVersionNotSupported { version: (maj, min), profile } => {
                write!(fmt, "GL version {maj}.{min} is not supported for profile {profile:?}")
            }
            GlError::NSOpenGLPixelFormatInitFailed => {
                fmt.write_str("NSOpenGLPixelFormat initialization failed")
            }
            GlError::NSOpenGLViewInitFailed => fmt.write_str("NSOpenGLView initialization failed"),
            GlError::OpenGlBundleNotFound => {
                fmt.write_str("Could not find bundle com.apple.opengl")
            }
        }
    }
}

#[derive(Clone)]
pub struct GlContext {
    pub(crate) view: Retained<NSOpenGLView>,
    context: Retained<NSOpenGLContext>,
    gl_bundle: CFRetained<CFBundle>,
}

impl GlContext {
    pub(crate) fn create(
        parent_view: &NSView, config: GlConfig, marker: MainThreadMarker,
    ) -> Result<GlContext> {
        let version = if config.version < (3, 2) && config.profile == Profile::Compatibility {
            NSOpenGLProfileVersionLegacy
        } else if config.version == (3, 2) && config.profile == Profile::Core {
            NSOpenGLProfileVersion3_2Core
        } else if config.version > (3, 2) && config.profile == Profile::Core {
            NSOpenGLProfileVersion4_1Core
        } else {
            return Err(GlError::GlVersionNotSupported {
                version: config.version,
                profile: config.profile,
            }
            .into());
        };

        #[rustfmt::skip]
        let mut attrs = vec![
            NSOpenGLPFAOpenGLProfile, version,
            NSOpenGLPFAColorSize, (config.red_bits + config.blue_bits + config.green_bits) as u32,
            NSOpenGLPFAAlphaSize, config.alpha_bits as u32,
            NSOpenGLPFADepthSize, config.depth_bits as u32,
            NSOpenGLPFAStencilSize, config.stencil_bits as u32,
            NSOpenGLPFAAccelerated,
        ];

        if let Some(samples) = config.samples {
            #[rustfmt::skip]
            attrs.extend_from_slice(&[
                NSOpenGLPFAMultisample,
                NSOpenGLPFASampleBuffers, 1,
                NSOpenGLPFASamples, samples as u32,
            ]);
        }

        if config.double_buffer {
            attrs.push(NSOpenGLPFADoubleBuffer);
        }

        attrs.push(0);

        let Some(attrs) = NonNull::new(attrs.as_mut_ptr()) else {
            // PANIC: This cannot panic, as the pointer comes from the vec
            unreachable!()
        };

        // SAFETY: Attribs pointer is valid (coming from the above vec) and null-terminated
        let pixel_format =
            unsafe { NSOpenGLPixelFormat::initWithAttributes(NSOpenGLPixelFormat::alloc(), attrs) }
                .ok_or(GlError::NSOpenGLPixelFormatInitFailed)?;

        let view = NSOpenGLView::initWithFrame_pixelFormat(
            NSOpenGLView::alloc(marker),
            parent_view.frame(),
            Some(&pixel_format),
        )
        .ok_or(GlError::NSOpenGLViewInitFailed)?;

        view.setWantsBestResolutionOpenGLSurface(true);

        view.display();
        parent_view.addSubview(&view);

        // NSOpenGlView::openGLContext is not documented to possibly return NULL.
        let Some(context) = view.openGLContext() else { unreachable!() };

        let value = config.vsync as i32;

        // SAFETY: pointer is a valid &i32, and is valid for SwapInterval
        unsafe {
            context.setValues_forParameter((&value).into(), NSOpenGLContextParameter::SwapInterval);
        }

        let framework_name = CFString::from_static_str("com.apple.opengl");
        let gl_bundle = CFBundle::bundle_with_identifier(Some(&framework_name))
            .ok_or(GlError::OpenGlBundleNotFound)?;

        Ok(GlContext { view, context, gl_bundle })
    }

    pub unsafe fn make_current(&self) -> Result<()> {
        self.context.makeCurrentContext();
        Ok(())
    }

    pub unsafe fn make_not_current(&self) -> Result<()> {
        NSOpenGLContext::clearCurrentContext();
        Ok(())
    }

    pub fn get_proc_address(&self, symbol: &CStr) -> *const c_void {
        // PANIC: CStr alloc can not be longer than isize
        let Ok(bytes_count) = symbol.count_bytes().try_into() else { unreachable!() };

        // SAFETY: The string pointer is valid
        let symbol_name = unsafe {
            CFString::with_bytes(
                None,
                symbol.as_ptr().cast(),
                bytes_count,
                CFStringBuiltInEncodings::EncodingUTF8.0,
                false,
            )
        };

        let Some(symbol_name) = symbol_name else {
            warn!("Failed to create CFString for symbol {:?}", symbol);
            return core::ptr::null();
        };

        self.gl_bundle.function_pointer_for_name(Some(&symbol_name))
    }

    pub fn swap_buffers(&self) -> Result<()> {
        self.context.flushBuffer();
        self.view.setNeedsDisplay(true);
        Ok(())
    }

    /// On macOS the `NSOpenGLView` needs to be resized separtely from our main view.
    pub(crate) fn resize(&self, size: NSSize) {
        self.view.setFrameSize(size);
        self.view.setNeedsDisplay(true);
    }
}
