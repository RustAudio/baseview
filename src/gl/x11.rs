use std::ffi::{c_void, CString};
use std::os::raw::{c_int, c_ulong};

use x11::glx;
use x11::xlib;

use super::{GlConfig, GlError, Profile};

mod errors;

#[derive(Debug)]
pub enum CreationFailedError {
    InvalidFBConfig,
    NoVisual,
    GetProcAddressFailed,
    MakeCurrentFailed,
    ContextCreationFailed,
    X11Error(errors::XLibError),
}

impl From<errors::XLibError> for GlError {
    fn from(e: errors::XLibError) -> Self {
        GlError::CreationFailed(CreationFailedError::X11Error(e))
    }
}

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/GLX_ARB_create_context.txt

type GlXCreateContextAttribsARB = unsafe extern "C" fn(
    dpy: *mut xlib::Display,
    fbc: glx::GLXFBConfig,
    share_context: glx::GLXContext,
    direct: xlib::Bool,
    attribs: *const c_int,
) -> glx::GLXContext;

// See https://www.khronos.org/registry/OpenGL/extensions/EXT/EXT_swap_control.txt

type GlXSwapIntervalEXT =
    unsafe extern "C" fn(dpy: *mut xlib::Display, drawable: glx::GLXDrawable, interval: i32);

// See https://www.khronos.org/registry/OpenGL/extensions/ARB/ARB_framebuffer_sRGB.txt

const GLX_FRAMEBUFFER_SRGB_CAPABLE_ARB: i32 = 0x20B2;

fn get_proc_address(symbol: &str) -> *const c_void {
    let symbol = CString::new(symbol).unwrap();
    unsafe { glx::glXGetProcAddress(symbol.as_ptr() as *const u8).unwrap() as *const c_void }
}

pub struct GlContext {
    window: c_ulong,
    display: *mut xlib::_XDisplay,
    context: glx::GLXContext,
}

/// The frame buffer configuration along with the general OpenGL configuration to somewhat minimize
/// misuse.
pub struct FbConfig {
    gl_config: GlConfig,
    fb_config: *mut glx::__GLXFBConfigRec,
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
    pub unsafe fn create(
        window: c_ulong, display: *mut xlib::_XDisplay, config: FbConfig,
    ) -> Result<GlContext, GlError> {
        if display.is_null() {
            return Err(GlError::InvalidWindowHandle);
        }

        errors::XErrorHandler::handle(display, |error_handler| {
            #[allow(non_snake_case)]
            let glXCreateContextAttribsARB: GlXCreateContextAttribsARB = {
                let addr = get_proc_address("glXCreateContextAttribsARB");
                if addr.is_null() {
                    return Err(GlError::CreationFailed(CreationFailedError::GetProcAddressFailed));
                } else {
                    std::mem::transmute(addr)
                }
            };

            #[allow(non_snake_case)]
            let glXSwapIntervalEXT: GlXSwapIntervalEXT = {
                let addr = get_proc_address("glXSwapIntervalEXT");
                if addr.is_null() {
                    return Err(GlError::CreationFailed(CreationFailedError::GetProcAddressFailed));
                } else {
                    std::mem::transmute(addr)
                }
            };

            error_handler.check()?;

            let profile_mask = match config.gl_config.profile {
                Profile::Core => glx::arb::GLX_CONTEXT_CORE_PROFILE_BIT_ARB,
                Profile::Compatibility => glx::arb::GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB,
            };

            #[rustfmt::skip]
                let ctx_attribs = [
                glx::arb::GLX_CONTEXT_MAJOR_VERSION_ARB, config.gl_config.version.0 as i32,
                glx::arb::GLX_CONTEXT_MINOR_VERSION_ARB, config.gl_config.version.1 as i32,
                glx::arb::GLX_CONTEXT_PROFILE_MASK_ARB, profile_mask,
                0,
            ];

            let context = glXCreateContextAttribsARB(
                display,
                config.fb_config,
                std::ptr::null_mut(),
                1,
                ctx_attribs.as_ptr(),
            );

            error_handler.check()?;

            if context.is_null() {
                return Err(GlError::CreationFailed(CreationFailedError::ContextCreationFailed));
            }

            let res = glx::glXMakeCurrent(display, window, context);
            error_handler.check()?;
            if res == 0 {
                return Err(GlError::CreationFailed(CreationFailedError::MakeCurrentFailed));
            }

            glXSwapIntervalEXT(display, window, config.gl_config.vsync as i32);
            error_handler.check()?;

            if glx::glXMakeCurrent(display, 0, std::ptr::null_mut()) == 0 {
                error_handler.check()?;
                return Err(GlError::CreationFailed(CreationFailedError::MakeCurrentFailed));
            }

            Ok(GlContext { window, display, context })
        })
    }

    /// Find a matching framebuffer config and window visual for the given OpenGL configuration.
    /// This needs to be passed to [Self::create] along with a handle to a window that was created
    /// using the visual also returned from this function.
    pub unsafe fn get_fb_config_and_visual(
        display: *mut xlib::_XDisplay, config: GlConfig,
    ) -> Result<(FbConfig, WindowConfig), GlError> {
        errors::XErrorHandler::handle(display, |error_handler| {
            let screen = xlib::XDefaultScreen(display);

            #[rustfmt::skip]
                let fb_attribs = [
                glx::GLX_X_RENDERABLE, 1,
                glx::GLX_X_VISUAL_TYPE, glx::GLX_TRUE_COLOR,
                glx::GLX_DRAWABLE_TYPE, glx::GLX_WINDOW_BIT,
                glx::GLX_RENDER_TYPE, glx::GLX_RGBA_BIT,
                glx::GLX_RED_SIZE, config.red_bits as i32,
                glx::GLX_GREEN_SIZE, config.green_bits as i32,
                glx::GLX_BLUE_SIZE, config.blue_bits as i32,
                glx::GLX_ALPHA_SIZE, config.alpha_bits as i32,
                glx::GLX_DEPTH_SIZE, config.depth_bits as i32,
                glx::GLX_STENCIL_SIZE, config.stencil_bits as i32,
                glx::GLX_DOUBLEBUFFER, config.double_buffer as i32,
                glx::GLX_SAMPLE_BUFFERS, config.samples.is_some() as i32,
                glx::GLX_SAMPLES, config.samples.unwrap_or(0) as i32,
                GLX_FRAMEBUFFER_SRGB_CAPABLE_ARB, config.srgb as i32,
                0,
            ];

            let mut n_configs = 0;
            let fb_config =
                glx::glXChooseFBConfig(display, screen, fb_attribs.as_ptr(), &mut n_configs);

            error_handler.check()?;
            if n_configs <= 0 || fb_config.is_null() {
                return Err(GlError::CreationFailed(CreationFailedError::InvalidFBConfig));
            }

            // Now that we have a matching framebuffer config, we need to know which visual matches
            // thsi config so the window is compatible with the OpenGL context we're about to create
            let fb_config = *fb_config;
            let visual = glx::glXGetVisualFromFBConfig(display, fb_config);
            if visual.is_null() {
                return Err(GlError::CreationFailed(CreationFailedError::NoVisual));
            }

            Ok((
                FbConfig { fb_config, gl_config: config },
                WindowConfig { depth: (*visual).depth as u8, visual: (*visual).visualid as u32 },
            ))
        })
    }

    pub unsafe fn make_current(&self) {
        errors::XErrorHandler::handle(self.display, |error_handler| {
            let res = glx::glXMakeCurrent(self.display, self.window, self.context);
            error_handler.check().unwrap();
            if res == 0 {
                panic!("make_current failed")
            }
        })
    }

    pub unsafe fn make_not_current(&self) {
        errors::XErrorHandler::handle(self.display, |error_handler| {
            let res = glx::glXMakeCurrent(self.display, 0, std::ptr::null_mut());
            error_handler.check().unwrap();
            if res == 0 {
                panic!("make_not_current failed")
            }
        })
    }

    pub fn get_proc_address(&self, symbol: &str) -> *const c_void {
        get_proc_address(symbol)
    }

    pub fn swap_buffers(&self) {
        unsafe {
            errors::XErrorHandler::handle(self.display, |error_handler| {
                glx::glXSwapBuffers(self.display, self.window);
                error_handler.check().unwrap();
            })
        }
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {}
}
