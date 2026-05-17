use super::{GlConfig, GlError};
use crate::x11::XcbConnection;
use std::ffi::{c_void, CString};
use std::os::raw::c_ulong;
use std::rc::Rc;
use x11_dl::error::OpenError;
use x11_dl::glx::GLXContext;

use crate::wrappers::glx::*;
use crate::wrappers::xlib::{XErrorHandler, XLibError};

#[derive(Debug)]
pub enum CreationFailedError {
    NoValidFBConfig,
    NoVisual,
    GetProcAddressFailed,
    MakeCurrentFailed,
    ContextCreationFailed,
    X11Error(XLibError),
    OpenError(OpenError),
}

impl From<XLibError> for GlError {
    fn from(e: XLibError) -> Self {
        GlError::CreationFailed(CreationFailedError::X11Error(e))
    }
}

impl From<OpenError> for GlError {
    fn from(e: OpenError) -> Self {
        GlError::CreationFailed(CreationFailedError::OpenError(e))
    }
}

pub struct GlContext {
    glx: Glx,
    window: c_ulong,
    connection: Rc<XcbConnection>,
    context: GLXContext,
}

/// The frame buffer configuration along with the general OpenGL configuration to somewhat minimize
/// misuse.
pub struct FbConfig {
    gl_config: GlConfig,
    fb_config: GlxFbConfig,
}

/// The configuration a window should be created with after calling
/// [GlContext::get_fb_config_and_visual].
pub struct WindowConfig {
    pub depth: u8,
    pub visual: u32,
}

impl GlContext {
    /// Creating an OpenGL context under X11 works slightly different. Different OpenGL
    /// configurations require different framebuffer configurations, and to be able to use that
    /// context with a window the window needs to be created with a matching visual. This means that
    /// you need to decide on the framebuffer config before creating the window, ask the X11 server
    /// for a matching visual for that framebuffer config, crate the window with that visual, and
    /// only then create the OpenGL context.
    ///
    /// Use [Self::get_fb_config_and_visual] to create both of these things.
    pub fn create(
        window: c_ulong, connection: Rc<XcbConnection>, config: FbConfig,
    ) -> Result<GlContext, GlError> {
        let glx = Glx::open()?;

        let xlib_connection = connection.conn.xlib_connection();

        XErrorHandler::handle(xlib_connection, |error_handler| {
            let Some(create_context) = glx.get_glx_create_context_attribs_arb() else {
                return Err(GlError::CreationFailed(CreationFailedError::GetProcAddressFailed));
            };

            let Some(swap_interval) = glx.get_glx_swap_interval_ext() else {
                return Err(GlError::CreationFailed(CreationFailedError::GetProcAddressFailed));
            };

            let context = create_context.call(
                xlib_connection,
                &config.gl_config,
                config.fb_config,
                error_handler,
            )?;

            // Create context object here so that error or panic will properly free the context
            let context = GlContext { glx, window, connection: Rc::clone(&connection), context };

            unsafe {
                context.glx.with_current_context(
                    xlib_connection,
                    window,
                    context.context,
                    error_handler,
                    || {
                        swap_interval(xlib_connection.dpy(), window, config.gl_config.vsync as i32);
                        error_handler.check()
                    },
                )??;
            }

            Ok(context)
        })
    }

    /// Find a matching framebuffer config and window visual for the given OpenGL configuration.
    /// This needs to be passed to [Self::create] along with a handle to a window that was created
    /// using the visual also returned from this function.
    pub fn get_fb_config_and_visual(
        connection: &XcbConnection, config: GlConfig,
    ) -> Result<(FbConfig, WindowConfig), GlError> {
        let glx = Glx::open()?;

        let xlib_connection = connection.conn.xlib_connection();

        XErrorHandler::handle(xlib_connection, |error_handler| {
            let fb_config = glx.choose_best_fb_config(
                xlib_connection,
                &config,
                connection.screen,
                error_handler,
            )?;

            // Now that we have a matching framebuffer config, we need to know which visual matches
            // this config so the window is compatible with the OpenGL context we're about to create
            let visual =
                glx.get_visual_from_fb_config(xlib_connection, fb_config, error_handler)?;

            Ok((
                FbConfig { fb_config, gl_config: config },
                WindowConfig { depth: visual.depth as u8, visual: visual.visualid as u32 },
            ))
        })
    }

    pub unsafe fn make_current(&self) {
        XErrorHandler::handle(self.connection.conn.xlib_connection(), |error_handler| {
            self.glx
                .make_current(
                    self.connection.conn.xlib_connection(),
                    self.window,
                    self.context,
                    error_handler,
                )
                .unwrap();
        })
    }

    pub unsafe fn make_not_current(&self) {
        XErrorHandler::handle(self.connection.conn.xlib_connection(), |error_handler| {
            self.glx.clear_current(self.connection.conn.xlib_connection(), error_handler).unwrap();
        })
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let symbol = CString::new(symbol).unwrap();

        match self.glx.get_proc_address(&symbol) {
            Some(ptr) => ptr.as_ptr(),
            None => std::ptr::null(),
        }
    }

    pub fn swap_buffers(&self) {
        XErrorHandler::handle(self.connection.conn.xlib_connection(), |error_handler| {
            self.glx
                .swap_buffers(self.connection.conn.xlib_connection(), self.window, error_handler)
                .unwrap()
        })
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        unsafe { self.glx.destroy_context(self.connection.conn.xlib_connection(), self.context) }
    }
}
