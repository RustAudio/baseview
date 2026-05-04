#![allow(deprecated)] // OpenGL is deprecated on macOS

use super::{GlConfig, GlError, Profile};
use objc2::rc::Retained;
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSOpenGLContext, NSOpenGLContextParameter, NSOpenGLPFAAccelerated, NSOpenGLPFAAlphaSize,
    NSOpenGLPFAColorSize, NSOpenGLPFADepthSize, NSOpenGLPFADoubleBuffer, NSOpenGLPFAMultisample,
    NSOpenGLPFAOpenGLProfile, NSOpenGLPFASampleBuffers, NSOpenGLPFASamples, NSOpenGLPFAStencilSize,
    NSOpenGLPixelFormat, NSOpenGLProfileVersion3_2Core, NSOpenGLProfileVersion4_1Core,
    NSOpenGLProfileVersionLegacy, NSOpenGLView, NSView,
};
use objc2_core_foundation::{CFBundle, CFString};
use objc2_foundation::NSSize;
use raw_window_handle::RawWindowHandle;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::str::FromStr;

pub type CreationFailedError = ();
pub struct GlContext {
    view: Retained<NSOpenGLView>,
    context: Retained<NSOpenGLContext>,
}

impl GlContext {
    pub unsafe fn create(parent: &RawWindowHandle, config: GlConfig) -> Result<GlContext, GlError> {
        let handle = if let RawWindowHandle::AppKit(handle) = parent {
            handle
        } else {
            return Err(GlError::InvalidWindowHandle);
        };

        let parent_view = handle.ns_view.cast::<NSView>();
        let Some(parent_view) = parent_view.as_ref() else {
            return Err(GlError::InvalidWindowHandle);
        };

        let version = if config.version < (3, 2) && config.profile == Profile::Compatibility {
            NSOpenGLProfileVersionLegacy
        } else if config.version == (3, 2) && config.profile == Profile::Core {
            NSOpenGLProfileVersion3_2Core
        } else if config.version > (3, 2) && config.profile == Profile::Core {
            NSOpenGLProfileVersion4_1Core
        } else {
            return Err(GlError::VersionNotSupported);
        };

        #[rustfmt::skip]
        let mut attrs = vec![
            NSOpenGLPFAOpenGLProfile as u32, version as u32,
            NSOpenGLPFAColorSize as u32, (config.red_bits + config.blue_bits + config.green_bits) as u32,
            NSOpenGLPFAAlphaSize as u32, config.alpha_bits as u32,
            NSOpenGLPFADepthSize as u32, config.depth_bits as u32,
            NSOpenGLPFAStencilSize as u32, config.stencil_bits as u32,
            NSOpenGLPFAAccelerated as u32,
        ];

        if config.samples.is_some() {
            #[rustfmt::skip]
            attrs.extend_from_slice(&[
                NSOpenGLPFAMultisample as u32,
                NSOpenGLPFASampleBuffers as u32, 1,
                NSOpenGLPFASamples as u32, config.samples.unwrap() as u32,
            ]);
        }

        if config.double_buffer {
            attrs.push(NSOpenGLPFADoubleBuffer as u32);
        }

        attrs.push(0);

        let pixel_format = NSOpenGLPixelFormat::initWithAttributes(
            NSOpenGLPixelFormat::alloc(),
            NonNull::new(attrs.as_mut_ptr()).unwrap(),
        )
        .ok_or(GlError::CreationFailed(()))?;

        let view = NSOpenGLView::initWithFrame_pixelFormat(
            NSOpenGLView::alloc(),
            parent_view.frame(),
            Some(&pixel_format),
        )
        .ok_or(GlError::CreationFailed(()))?;

        view.setWantsBestResolutionOpenGLSurface(true);

        view.display();
        parent_view.addSubview(&view);

        let context = view.openGLContext().ok_or(GlError::CreationFailed(()))?;

        let value = config.vsync as i32;

        context.setValues_forParameter((&value).into(), NSOpenGLContextParameter::SwapInterval);

        Ok(GlContext { view, context })
    }

    pub unsafe fn make_current(&self) {
        self.context.makeCurrentContext();
    }

    pub unsafe fn make_not_current(&self) {
        if Some(self.context) == NSOpenGLContext::currentContext() {
            NSOpenGLContext::clearCurrentContext();
        }
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol_name = CFString::from_str(symbol);
        let framework_name = CFString::from_static_str("com.apple.opengl");
        let framework = CFBundle::bundle_with_identifier(Some(&framework_name)).unwrap();

        CFBundle::function_pointer_for_name(&framework, Some(&symbol_name))
    }

    pub fn swap_buffers(&self) {
        self.context.flushBuffer();
        self.view.setNeedsDisplay(true);
    }

    /// On macOS the `NSOpenGLView` needs to be resized separtely from our main view.
    pub(crate) fn resize(&self, size: NSSize) {
        self.view.setFrameSize(size);
        self.view.setNeedsDisplay(true);
    }

    /// Pointer to the `NSOpenGLView` this context renders into. Used by
    /// the parent `NSView`'s `hitTest:` override to collapse hits on the
    /// render subview to the parent, so AppKit routes `mouseDown:` on
    /// first click in non-key windows.
    pub(crate) fn ns_view(&self) -> &NSOpenGLView {
        &self.view
    }
}
