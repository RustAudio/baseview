use super::prelude::*;
use crate::gl::*;
use crate::wrappers::glx::*;
use std::error::Error;

use crate::platform::gl::egl::EglGlContext;
use crate::platform::gl::glx::GlxGlContext;
use crate::platform::x11::xcb_window::XcbWindow;
use crate::wrappers::egl::*;
use std::ffi::{c_void, CStr};
use std::rc::Rc;
use x11rb::protocol::xproto::Visualid;

mod egl;
mod error;
mod glx;

pub use error::GlCreationFailedError;
pub type PlatformGlContext = Rc<GlContextInner>;

pub enum GlContextInner {
    Glx(GlxGlContext),
    Egl(EglGlContext),
}

/// The frame buffer configuration along with the general OpenGL configuration to somewhat minimize
/// misuse.
pub struct FbConfig {
    gl_config: GlConfig,
    fb_config: FbConfigInner,
}

enum FbConfigInner {
    Glx { glx: Glx, config: GlxFbConfig },
    Egl { display: EglDisplay, config: EglConfig },
}

/// The configuration a window should be created with after calling
/// [GlContextInner::get_fb_config_and_visual].
pub struct WindowConfig {
    pub depth: u8,
    pub visual: u32,
}

impl GlContextInner {
    /// Creating an OpenGL context under X11 works slightly different. Different OpenGL
    /// configurations require different framebuffer configurations, and to be able to use that
    /// context with a window the window needs to be created with a matching visual. This means that
    /// you need to decide on the framebuffer config before creating the window, ask the X11 server
    /// for a matching visual for that framebuffer config, crate the window with that visual, and
    /// only then create the OpenGL context.
    ///
    /// Use [Self::get_fb_config_and_visual] to create both of these things.
    pub fn create(
        window: &XcbWindow, connection: &Rc<X11Connection>, fb_config: FbConfig,
    ) -> PlatformResult<Rc<GlContextInner>> {
        let inner =
            match fb_config.fb_config {
                FbConfigInner::Glx { glx, config } => GlContextInner::Glx(GlxGlContext::create(
                    window,
                    connection,
                    fb_config.gl_config,
                    config,
                    glx,
                )?),
                FbConfigInner::Egl { display, config } => GlContextInner::Egl(
                    EglGlContext::create(window, &fb_config.gl_config, config, display)?,
                ),
            };

        Ok(Rc::new(inner))
    }

    /// Find a matching framebuffer config and window visual for the given OpenGL configuration.
    /// This needs to be passed to [Self::create] along with a handle to a window that was created
    /// using the visual also returned from this function.
    pub fn get_fb_config_and_visual(
        connection: &Rc<X11Connection>, config: GlConfig,
    ) -> PlatformResult<(FbConfig, WindowConfig)> {
        EglGlContext::get_fb_config_and_visual(connection, config.clone())
            .or_else(|_| GlxGlContext::get_fb_config_and_visual(connection, config))
    }

    pub unsafe fn make_current(&self) -> PlatformResult<()> {
        match self {
            GlContextInner::Glx(glx) => glx.make_current(),
            GlContextInner::Egl(egl) => egl.make_current(),
        }
    }

    pub unsafe fn make_not_current(&self) -> PlatformResult<()> {
        match self {
            GlContextInner::Glx(glx) => glx.make_not_current(),
            GlContextInner::Egl(egl) => egl.make_not_current(),
        }
    }

    pub fn get_proc_address(&self, symbol: &CStr) -> *const c_void {
        match self {
            GlContextInner::Glx(glx) => glx.get_proc_address(symbol),
            GlContextInner::Egl(egl) => egl.get_proc_address(symbol),
        }
    }

    pub fn swap_buffers(&self) -> PlatformResult<()> {
        match self {
            GlContextInner::Glx(glx) => glx.swap_buffers(),
            GlContextInner::Egl(egl) => egl.swap_buffers(),
        }
    }
}
