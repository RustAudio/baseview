use std::ffi::c_void;
use std::str::FromStr;

use raw_window_handle::RawWindowHandle;

use cocoa::appkit::{
    NSOpenGLContext, NSOpenGLContextParameter, NSOpenGLPFAAccelerated, NSOpenGLPFAAlphaSize,
    NSOpenGLPFAColorSize, NSOpenGLPFADepthSize, NSOpenGLPFADoubleBuffer, NSOpenGLPFAMultisample,
    NSOpenGLPFAOpenGLProfile, NSOpenGLPFASampleBuffers, NSOpenGLPFASamples, NSOpenGLPFAStencilSize,
    NSOpenGLPixelFormat, NSOpenGLProfileVersion3_2Core, NSOpenGLProfileVersion4_1Core,
    NSOpenGLProfileVersionLegacy, NSOpenGLView, NSView,
};
use cocoa::base::{id, nil, YES};
use cocoa::foundation::NSSize;

use core_foundation::base::TCFType;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::string::CFString;

use objc::{msg_send, sel, sel_impl};

use super::{GlConfig, GlError, Profile};

pub type CreationFailedError = ();
pub struct GlContext {
    view: id,
    context: id,
}

impl GlContext {
    pub unsafe fn create(parent: &RawWindowHandle, config: GlConfig) -> Result<GlContext, GlError> {
        let handle = if let RawWindowHandle::AppKit(handle) = parent {
            handle
        } else {
            return Err(GlError::InvalidWindowHandle);
        };

        if handle.ns_view.is_null() {
            return Err(GlError::InvalidWindowHandle);
        }

        let parent_view = handle.ns_view as id;

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

        let pixel_format = NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&attrs);

        if pixel_format == nil {
            return Err(GlError::CreationFailed(()));
        }

        let view =
            NSOpenGLView::alloc(nil).initWithFrame_pixelFormat_(parent_view.frame(), pixel_format);

        if view == nil {
            return Err(GlError::CreationFailed(()));
        }

        view.setWantsBestResolutionOpenGLSurface_(YES);

        let () = msg_send![view, retain];
        NSOpenGLView::display_(view);
        parent_view.addSubview_(view);

        let context: id = msg_send![view, openGLContext];
        let () = msg_send![context, retain];

        context.setValues_forParameter_(
            &(config.vsync as i32),
            NSOpenGLContextParameter::NSOpenGLCPSwapInterval,
        );

        let () = msg_send![pixel_format, release];

        Ok(GlContext { view, context })
    }

    pub unsafe fn make_current(&self) {
        self.context.makeCurrentContext();
    }

    pub unsafe fn make_not_current(&self) {
        NSOpenGLContext::clearCurrentContext(self.context);
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol_name = CFString::from_str(symbol).unwrap();
        let framework_name = CFString::from_str("com.apple.opengl").unwrap();
        let framework =
            unsafe { CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef()) };

        unsafe { CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef()) }
    }

    pub fn swap_buffers(&self) {
        unsafe {
            self.context.flushBuffer();
            let () = msg_send![self.view, setNeedsDisplay: YES];
        }
    }

    /// On macOS the `NSOpenGLView` needs to be resized separtely from our main view.
    pub(crate) fn resize(&self, size: NSSize) {
        unsafe { NSView::setFrameSize(self.view, size) };
        unsafe {
            let _: () = msg_send![self.view, setNeedsDisplay: YES];
        }
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.context, release];
            let () = msg_send![self.view, release];
        }
    }
}
