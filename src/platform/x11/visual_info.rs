use super::prelude::*;
use super::x11_connection::X11Connection;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Colormap, ColormapAlloc, ConnectionExt, Screen, VisualClass, Visualid,
};
use x11rb::COPY_FROM_PARENT;

pub struct WindowVisualConfig {
    pub visual_depth: u8,
    pub visual_id: Visualid,
    pub color_map: Option<Colormap>,

    #[cfg(feature = "opengl")]
    pub fb_config: Option<super::gl::FbConfig>,
}

// TODO: make visual negotiation actually check all of a visual's parameters
impl WindowVisualConfig {
    pub fn find_best_visual_config(
        connection: &Rc<X11Connection>, settings: &mut WindowSettings,
    ) -> PlatformResult<Self> {
        #[cfg(feature = "opengl")]
        if let Some(gl_config) = settings.gl_config.take() {
            let (fb_config, window_config) =
                GlContextInner::get_fb_config_and_visual(connection, gl_config)?;

            return Ok(Self {
                fb_config: Some(fb_config),
                visual_depth: window_config.depth,
                visual_id: window_config.visual,
                color_map: Some(create_color_map(connection, window_config.visual)?),
            });
        }

        #[cfg(not(feature = "opengl"))]
        let _ = settings;

        match find_visual_for_depth(connection.default_screen(), 32) {
            None => Ok(Self::copy_from_parent()),
            Some(visual_id) => Ok(Self {
                #[cfg(feature = "opengl")]
                fb_config: None,
                visual_id,
                visual_depth: 32,
                color_map: Some(create_color_map(connection, visual_id)?),
            }),
        }
    }

    const fn copy_from_parent() -> Self {
        Self {
            #[cfg(feature = "opengl")]
            fb_config: None,
            visual_depth: COPY_FROM_PARENT as u8,
            visual_id: COPY_FROM_PARENT,
            color_map: None,
        }
    }

    #[cfg(feature = "opengl")]
    pub fn make_gl_context(
        self, window: &XcbWindow, connection: &Rc<X11Connection>,
    ) -> PlatformResult<Option<Rc<GlContextInner>>> {
        match self.fb_config {
            None => Ok(None),
            Some(fb_config) => Ok(Some(GlContextInner::create(window, connection, fb_config)?)),
        }
    }
}

// For this 32-bit depth to work, you also need to define a color map and set a border
// pixel: https://cgit.freedesktop.org/xorg/xserver/tree/dix/window.c#n818
fn create_color_map(connection: &X11Connection, visual_id: Visualid) -> PlatformResult<Colormap> {
    let colormap = connection.conn.generate_id()?;
    connection.conn.create_colormap(
        ColormapAlloc::NONE,
        colormap,
        connection.default_screen().root,
        visual_id,
    )?;

    Ok(colormap)
}

fn find_visual_for_depth(screen: &Screen, depth: u8) -> Option<Visualid> {
    for candidate_depth in &screen.allowed_depths {
        if candidate_depth.depth != depth {
            continue;
        }

        for candidate_visual in &candidate_depth.visuals {
            if candidate_visual.class == VisualClass::TRUE_COLOR {
                return Some(candidate_visual.visual_id);
            }
        }
    }

    None
}
