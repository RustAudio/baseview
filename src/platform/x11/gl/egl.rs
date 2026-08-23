use crate::gl::GlConfig;
use crate::platform::gl::{FbConfig, FbConfigInner, WindowConfig};
use crate::platform::x11::xcb_window::XcbWindow;
use crate::platform::{PlatformError, X11Connection};
use crate::wrappers::egl::{Egl, EglConfig, EglDisplay};
use khronos_egl::EGLDisplay;
use std::num::NonZeroU32;
use std::rc::Rc;
use x11rb::protocol::xproto::Visualid;

pub struct EglGlContext {
    display: EGLDisplay,
    window: NonZeroU32,
    connection: Rc<X11Connection>,
}

impl EglGlContext {
    pub(crate) fn create(
        window: &XcbWindow, connection: Rc<X11Connection>, gl_config: GlConfig,
        egl_config: EglConfig, display: EglDisplay,
    ) -> Result<Self, PlatformError> {
        todo!()
    }
}

impl EglGlContext {
    pub fn get_fb_config_and_visual(
        connection: &X11Connection, gl_config: &GlConfig,
    ) -> Result<(FbConfig, WindowConfig), PlatformError> {
        let egl = Egl::open()?;
        let display = egl.create_display(&connection.conn)?;

        let config = display.choose_config(&gl_config)?.unwrap();
        let visual = config.get_visual_id(&display)?;

        let depth = Self::find_visual_depth_for_id(&connection, visual).unwrap(); // TODO

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
}
