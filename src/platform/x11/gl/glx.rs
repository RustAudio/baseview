use super::*;
use crate::gl::GlConfig;
use crate::platform::gl::CreationFailedError;
use crate::platform::x11::xcb_window::XcbWindow;
use crate::platform::X11Connection;
use crate::wrappers::glx::{Glx, GlxFbConfig};
use crate::wrappers::xlib::XErrorHandler;
use std::ffi::{c_ulong, c_void, CStr};
use std::num::NonZeroU32;
use std::rc::Rc;
use x11_dl::glx::GLXContext;

pub struct GlxGlContext {
    glx: Glx,
    window: NonZeroU32,
    connection: Rc<X11Connection>,
    context: GLXContext,
}

impl GlxGlContext {
    pub fn create(
        window: &XcbWindow, connection: Rc<X11Connection>, gl_config: GlConfig,
        fb_config: GlxFbConfig, glx: Glx,
    ) -> Result<Self> {
        let xlib_connection = connection.conn.xlib_connection();

        XErrorHandler::handle(xlib_connection, |error_handler| {
            let Some(create_context) = glx.get_glx_create_context_attribs_arb() else {
                return Err(CreationFailedError::GetProcAddressFailed.into());
            };

            let context =
                create_context.call(xlib_connection, &gl_config, fb_config, error_handler)?;

            Ok(Self { glx, window: window.id(), connection: Rc::clone(&connection), context })
        })
    }

    pub fn get_fb_config_and_visual(
        connection: &X11Connection, config: &GlConfig,
    ) -> Result<(FbConfig, WindowConfig)> {
        let glx = Glx::open()?;

        let xlib_connection = connection.conn.xlib_connection();

        XErrorHandler::handle(xlib_connection, |error_handler| {
            let fb_config = glx.choose_best_fb_config(xlib_connection, &config, error_handler)?;

            // Now that we have a matching framebuffer config, we need to know which visual matches
            // this config so the window is compatible with the OpenGL context we're about to create
            let visual =
                glx.get_visual_from_fb_config(xlib_connection, fb_config, error_handler)?;

            Ok((
                FbConfig {
                    fb_config: FbConfigInner::Glx { config: fb_config, glx },
                    gl_config: *config,
                },
                WindowConfig { depth: visual.depth as u8, visual: visual.visualid as u32 },
            ))
        })
    }

    pub unsafe fn make_current(&self) -> Result<()> {
        XErrorHandler::handle(self.connection.conn.xlib_connection(), |error_handler| {
            self.glx.make_current(
                self.connection.conn.xlib_connection(),
                self.window_id(),
                self.context,
                error_handler,
            )
        })
    }

    pub unsafe fn make_not_current(&self) -> Result<()> {
        XErrorHandler::handle(self.connection.conn.xlib_connection(), |error_handler| {
            self.glx.clear_current(self.connection.conn.xlib_connection(), error_handler)
        })
    }

    fn window_id(&self) -> c_ulong {
        self.window.get().into()
    }

    pub fn get_proc_address(&self, symbol: &CStr) -> *const c_void {
        match self.glx.get_proc_address(symbol) {
            Some(ptr) => ptr.as_ptr(),
            None => std::ptr::null(),
        }
    }

    pub fn swap_buffers(&self) -> Result<()> {
        XErrorHandler::handle(self.connection.conn.xlib_connection(), |error_handler| {
            self.glx.swap_buffers(
                self.connection.conn.xlib_connection(),
                self.window_id(),
                error_handler,
            )
        })
    }
}

impl Drop for GlxGlContext {
    fn drop(&mut self) {
        unsafe { self.glx.destroy_context(self.connection.conn.xlib_connection(), self.context) }
    }
}
