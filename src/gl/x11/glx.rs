use crate::gl::platform::CreationFailedError;
use crate::gl::x11::errors::XErrorHandler;
use crate::gl::{GlConfig, GlError, Profile};
use crate::x11::XcbConnection;
use std::ffi::{c_ulong, c_void, CStr};
use std::os::raw::c_int;
use std::ptr::NonNull;
use x11_dl::glx::{arb::*, *};
use x11_dl::xlib;
use x11_dl::xlib::XVisualInfo;

/// See https://www.khronos.org/registry/OpenGL/extensions/ARB/GLX_ARB_create_context.txt
type GlXCreateContextAttribsARB = unsafe extern "C" fn(
    dpy: *mut xlib::Display,
    fbc: GLXFBConfig,
    share_context: GLXContext,
    direct: xlib::Bool,
    attribs: *const c_int,
) -> GLXContext;

/// See https://www.khronos.org/registry/OpenGL/extensions/EXT/EXT_swap_control.txt
type GlXSwapIntervalEXT =
    unsafe extern "C" fn(dpy: *mut xlib::Display, drawable: GLXDrawable, interval: i32);

/// See https://www.khronos.org/registry/OpenGL/extensions/ARB/ARB_framebuffer_sRGB.txt
const GLX_FRAMEBUFFER_SRGB_CAPABLE_ARB: i32 = 0x20B2;

pub struct Glx {
    inner: x11_dl::glx::Glx,
}

impl Glx {
    pub fn open() -> Result<Self, GlError> {
        Ok(Self { inner: x11_dl::glx::Glx::open()? })
    }

    fn get_fb_attribs(config: &GlConfig) -> [c_int; 29] {
        #[rustfmt::skip]
        let fb_attribs = [
            GLX_X_RENDERABLE, 1,
            GLX_X_VISUAL_TYPE, GLX_TRUE_COLOR,
            GLX_DRAWABLE_TYPE, GLX_WINDOW_BIT,
            GLX_RENDER_TYPE, GLX_RGBA_BIT,
            GLX_RED_SIZE, config.red_bits as i32,
            GLX_GREEN_SIZE, config.green_bits as i32,
            GLX_BLUE_SIZE, config.blue_bits as i32,
            GLX_ALPHA_SIZE, config.alpha_bits as i32,
            GLX_DEPTH_SIZE, config.depth_bits as i32,
            GLX_STENCIL_SIZE, config.stencil_bits as i32,
            GLX_DOUBLEBUFFER, config.double_buffer as i32,
            GLX_SAMPLE_BUFFERS, config.samples.is_some() as i32,
            GLX_SAMPLES, config.samples.unwrap_or(0) as i32,
            GLX_FRAMEBUFFER_SRGB_CAPABLE_ARB, config.srgb as i32,
            0,
        ];

        fb_attribs
    }

    pub fn choose_best_fb_config(
        &self, connection: &XcbConnection, config: &GlConfig, error_handler: &XErrorHandler,
    ) -> Result<GlxFbConfig, GlError> {
        let fb_attribs = Self::get_fb_attribs(config);

        let mut nelements = 0;
        // SAFETY: XcbConnection guarantees the inner dpy is valid.
        // The fb_attribs and nelements pointers come from references and are therefore valid.
        let result = unsafe {
            (self.inner.glXChooseFBConfig)(
                connection.dpy,
                connection.screen,
                fb_attribs.as_ptr(),
                &mut nelements,
            )
        };

        error_handler.check()?;
        if nelements == 0 || result.is_null() {
            return Err(GlError::CreationFailed(CreationFailedError::NoValidFBConfig));
        }

        // SAFETY: If nelements != 0, the result pointer is non-null, and no Xlib error occured, then
        // there must be at least one element in that array, which is valid for reads.
        let first_result = unsafe { result.read() };

        // SAFETY: for the same reasons above, glXChooseFBConfig returned a valid array
        // that we must free ourselves.
        unsafe { (connection.xlib.XFree)(result.cast()) };

        Ok(GlxFbConfig(first_result))
    }

    pub fn get_visual_from_fb_config(
        &self, connection: &XcbConnection, fb_config: GlxFbConfig, error_handler: &XErrorHandler,
    ) -> Result<XVisualInfo, GlError> {
        // SAFETY: XcbConnection guarantees the inner dpy is valid.
        let result = unsafe { (self.inner.glXGetVisualFromFBConfig)(connection.dpy, fb_config.0) };

        error_handler.check()?;
        if result.is_null() {
            return Err(GlError::CreationFailed(CreationFailedError::NoVisual));
        }

        // SAFETY: If the result pointer is non-null, and no Xlib error occured, then
        // glXGetVisualFromFBConfig must have returned a valid result.
        let visual = unsafe { result.read() };
        // SAFETY: for the same reasons above, glXGetVisualFromFBConfig returned a valid array
        // that we must free ourselves.
        unsafe { (connection.xlib.XFree)(result.cast()) };

        Ok(visual)
    }

    pub fn swap_buffers(
        &self, connection: &XcbConnection, window_id: c_ulong, error_handler: &XErrorHandler,
    ) -> Result<(), GlError> {
        // SAFETY: XcbConnection guarantees the inner dpy is valid.
        unsafe { (self.inner.glXSwapBuffers)(connection.dpy, window_id) };

        Ok(error_handler.check()?)
    }

    pub fn get_proc_address(&self, proc_name: &CStr) -> Option<NonNull<c_void>> {
        let result = unsafe { (self.inner.glXGetProcAddress)(proc_name.as_ptr().cast())? };

        NonNull::new(result as *mut c_void)
    }

    pub fn get_glx_swap_interval_ext(&self) -> Option<GlXSwapIntervalEXT> {
        let ptr = self.get_proc_address(c"glXSwapIntervalEXT")?;

        // SAFETY: NonNull is repr(transparent), GlXSwapIntervalEXT is the correct type for this function pointer
        Some(unsafe { core::mem::transmute::<NonNull<c_void>, GlXSwapIntervalEXT>(ptr) })
    }

    pub fn get_glx_create_context_attribs_arb(&self) -> Option<GlxCreateContextAttribsARB> {
        let ptr = self.get_proc_address(c"glXCreateContextAttribsARB")?;

        // SAFETY: NonNull is repr(transparent), GlxCreateContextAttribsARB is the correct type for this function pointer
        Some(GlxCreateContextAttribsARB(unsafe {
            core::mem::transmute::<NonNull<c_void>, GlXCreateContextAttribsARB>(ptr)
        }))
    }

    pub unsafe fn destroy_context(&self, connection: &XcbConnection, context: GLXContext) {
        // SAFETY:
        unsafe { (self.inner.glXDestroyContext)(connection.dpy, context) };
    }

    pub unsafe fn make_current(
        &self, connection: &XcbConnection, window_id: c_ulong, context: GLXContext,
        error_handler: &XErrorHandler,
    ) -> Result<(), GlError> {
        let res = unsafe { (self.inner.glXMakeCurrent)(connection.dpy, window_id, context) };

        error_handler.check()?;
        if res == 0 {
            return Err(GlError::CreationFailed(CreationFailedError::MakeCurrentFailed));
        }

        Ok(())
    }

    pub unsafe fn clear_current(
        &self, connection: &XcbConnection, error_handler: &XErrorHandler,
    ) -> Result<(), GlError> {
        self.make_current(connection, 0, core::ptr::null_mut(), error_handler)
    }

    pub unsafe fn with_current_context<T>(
        &self, connection: &XcbConnection, window_id: c_ulong, context: GLXContext,
        error_handler: &XErrorHandler, closure: impl FnOnce() -> T,
    ) -> Result<T, GlError> {
        self.make_current(connection, window_id, context, error_handler)?;

        // Using a "drop" allows us to clear the GL context even if the given closure panics
        let clearer = ContextClearOnDrop { glx: self, connection, error_handler };

        let result = closure();

        drop(clearer);

        Ok(result)
    }
}

pub struct ContextClearOnDrop<'a> {
    glx: &'a Glx,
    connection: &'a XcbConnection,
    error_handler: &'a XErrorHandler<'a>,
}

impl Drop for ContextClearOnDrop<'_> {
    fn drop(&mut self) {
        let _ = unsafe { self.glx.clear_current(self.connection, self.error_handler) };
    }
}

pub struct GlxCreateContextAttribsARB(GlXCreateContextAttribsARB);

impl GlxCreateContextAttribsARB {
    fn get_ctx_attribs(config: &GlConfig) -> [c_int; 7] {
        let profile_mask = match config.profile {
            Profile::Core => GLX_CONTEXT_CORE_PROFILE_BIT_ARB,
            Profile::Compatibility => GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB,
        };

        #[rustfmt::skip]
        let ctx_attribs = [
            GLX_CONTEXT_MAJOR_VERSION_ARB, config.version.0 as i32,
            GLX_CONTEXT_MINOR_VERSION_ARB, config.version.1 as i32,
            GLX_CONTEXT_PROFILE_MASK_ARB, profile_mask,
            0,
        ];

        ctx_attribs
    }

    pub fn call(
        &self, connection: &XcbConnection, gl_config: &GlConfig, glx_fb_config: GlxFbConfig,
        error_handler: &XErrorHandler,
    ) -> Result<GLXContext, GlError> {
        let ctx_attribs = Self::get_ctx_attribs(gl_config);

        let context = unsafe {
            self.0(connection.dpy, glx_fb_config.0, std::ptr::null_mut(), 1, ctx_attribs.as_ptr())
        };

        error_handler.check()?;

        if context.is_null() {
            return Err(GlError::CreationFailed(CreationFailedError::ContextCreationFailed));
        }

        Ok(context)
    }
}

/// Handle to a GLX Framebuffer configuration object.
///
/// These point to objects in a global table managed by glXChooseFBConfig, and are never destroyed.
/// Therefore, these have the 'static lifetime, and are always safe to use.
#[derive(Copy, Clone)]
pub struct GlxFbConfig(GLXFBConfig);
