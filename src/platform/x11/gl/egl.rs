use crate::gl::GlConfig;
use crate::platform::gl::{FbConfig, FbConfigInner, WindowConfig};
use crate::platform::x11::xcb_window::XcbWindow;
use crate::platform::{PlatformError, X11Connection};
use crate::wrappers::egl::{Egl, EglConfig, EglContext, EglDisplay, EglSurface};
use std::ffi::{c_void, CStr};
use std::rc::Rc;
use x11rb::protocol::xproto::Visualid;

pub struct EglGlContext {
    surface: EglSurface,
    context: EglContext,
}

impl EglGlContext {
    pub(crate) fn create(
        window: &XcbWindow, gl_config: &GlConfig, egl_config: EglConfig, display: EglDisplay,
    ) -> Result<Self, PlatformError> {
        let surface = display.create_surface(egl_config, window.id().get(), gl_config)?;
        let context = display
            .egl()
            .with_opengl(|bound| display.create_context(egl_config, bound, gl_config))??;

        Ok(Self { surface, context })
    }
}

impl EglGlContext {
    pub fn get_fb_config_and_visual(
        connection: &Rc<X11Connection>, gl_config: &GlConfig,
    ) -> Result<(FbConfig, WindowConfig), PlatformError> {
        let egl = Egl::open()?;
        let display = egl.create_display(connection)?; // TODO: check EGL version

        let config = display.choose_config(gl_config)?.unwrap();
        let visual = config.get_visual_id(&display)?;

        let depth = Self::find_visual_depth_for_id(connection, visual).unwrap(); // TODO

        let window_config = WindowConfig { depth, visual };
        let fb_config =
            FbConfig { gl_config: *gl_config, fb_config: FbConfigInner::Egl { display, config } };

        Ok((fb_config, window_config))
    }

    fn find_visual_depth_for_id(connection: &X11Connection, visual_id: Visualid) -> Option<u8> {
        connection
            .default_screen()
            .allowed_depths
            .iter()
            .find(|d| d.visuals.iter().any(|v| v.visual_id == visual_id))
            .map(|d| d.depth)
    }

    pub fn make_current(&self) -> Result<(), PlatformError> {
        self.context.make_current(&self.surface)?;

        Ok(())
    }

    pub fn make_not_current(&self) -> Result<(), PlatformError> {
        self.surface.display().egl().with_opengl(|gl| self.context.make_not_current(gl))??;

        Ok(())
    }

    pub fn get_proc_address(&self, symbol: &CStr) -> *const c_void {
        self.surface.display().egl().get_proc_address(symbol)
    }

    pub fn swap_buffers(&self) -> Result<(), PlatformError> {
        self.surface.swap_buffers()?;
        Ok(())
    }
}
