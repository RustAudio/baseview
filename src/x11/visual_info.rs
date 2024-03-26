use crate::x11::xcb_connection::XcbConnection;
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Colormap, ColormapAlloc, ConnectionExt, Screen, VisualClass, Visualid,
};
use x11rb::COPY_FROM_PARENT;

pub(super) struct WindowVisualConfig {
    #[cfg(feature = "opengl")]
    pub fb_config: Option<crate::gl::x11::FbConfig>,

    pub visual_depth: u8,
    pub visual_id: Visualid,
    pub is_copy_from_parent: bool,
}

// TODO: make visual negotiation actually check all of a visual's parameters
impl WindowVisualConfig {
    #[cfg(feature = "opengl")]
    pub fn find_best_visual_config_for_gl(
        connection: &XcbConnection, gl_config: Option<crate::gl::GlConfig>,
    ) -> Self {
        let Some(gl_config) = gl_config else { return Self::find_best_visual_config(connection) };

        // SAFETY: TODO
        let (fb_config, window_config) = unsafe {
            crate::gl::platform::GlContext::get_fb_config_and_visual(connection.dpy, gl_config)
        }
        .expect("Could not fetch framebuffer config");

        Self {
            fb_config: Some(fb_config),
            visual_depth: window_config.depth,
            visual_id: window_config.visual,
            is_copy_from_parent: false,
        }
    }

    pub fn find_best_visual_config(connection: &XcbConnection) -> Self {
        match find_visual_for_depth(connection.screen(), 32) {
            None => Self::copy_from_parent(),
            Some(visual_id) => Self {
                #[cfg(feature = "opengl")]
                fb_config: None,
                visual_id,
                visual_depth: 32,
                is_copy_from_parent: false,
            },
        }
    }

    const fn copy_from_parent() -> Self {
        Self {
            #[cfg(feature = "opengl")]
            fb_config: None,
            visual_depth: COPY_FROM_PARENT as u8,
            visual_id: COPY_FROM_PARENT,
            is_copy_from_parent: true,
        }
    }

    // For this 32-bit depth to work, you also need to define a color map and set a border
    // pixel: https://cgit.freedesktop.org/xorg/xserver/tree/dix/window.c#n818
    pub fn create_color_map(
        &self, connection: &XcbConnection,
    ) -> Result<Option<Colormap>, Box<dyn Error>> {
        if self.is_copy_from_parent {
            return Ok(None);
        }

        let colormap = connection.conn2.generate_id()?;
        connection.conn2.create_colormap(
            ColormapAlloc::NONE,
            colormap,
            connection.screen().root,
            self.visual_id,
        )?;

        Ok(Some(colormap))
    }
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
