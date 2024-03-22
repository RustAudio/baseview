use crate::x11::visual_info::WindowVisualConfig;
use crate::x11::xcb_connection::XcbConnection;
use crate::{MouseCursor, Size, WindowInfo, WindowOpenOptions, WindowScalePolicy};
use raw_window_handle::XcbWindowHandle;
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, CreateGCAux,
    CreateWindowAux, Drawable, EventMask, Gcontext, PropMode, Visualid, Window, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

/// Represents an actual X11 window (as opposed to the [`crate::x11::Window`] which also handles the
/// event loop for now).
pub(crate) struct X11Window {
    pub window_id: Window,
    pub dpi_scale_factor: f64,
    visual_id: Visualid,
    _graphics_context: Gcontext,

    #[cfg(feature = "opengl")]
    gl_context: Option<std::rc::Rc<crate::gl::x11::GlContext>>,
}

impl X11Window {
    pub fn new(
        connection: &XcbConnection, parent: Option<Drawable>, options: WindowOpenOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let parent = parent.unwrap_or_else(|| connection.screen().root);
        let _graphics_context = create_graphics_context(connection, parent)?;

        let scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => connection.get_scaling().unwrap_or(1.0),
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };

        let window_info = WindowInfo::from_logical_size(options.size, scaling);
        let physical_size = window_info.physical_size();

        #[cfg(feature = "opengl")]
        let visual_info =
            WindowVisualConfig::find_best_visual_config_for_gl(connection, options.gl_config)?;

        #[cfg(not(feature = "opengl"))]
        let visual_info = WindowVisualConfig::find_best_visual_config(connection)?;

        let window_id = connection.conn.generate_id()?;
        connection.conn.create_window(
            visual_info.visual_depth,
            window_id,
            parent,
            0,                           // x coordinate of the new window
            0,                           // y coordinate of the new window
            physical_size.width as u16,  // window width
            physical_size.height as u16, // window height
            0,                           // window border
            WindowClass::INPUT_OUTPUT,
            visual_info.visual_id,
            &CreateWindowAux::new()
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::POINTER_MOTION
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::KEY_PRESS
                        | EventMask::KEY_RELEASE
                        | EventMask::STRUCTURE_NOTIFY
                        | EventMask::ENTER_WINDOW
                        | EventMask::LEAVE_WINDOW,
                )
                // As mentioned above, these two values are needed to be able to create a window
                // with a depth of 32-bits when the parent window has a different depth
                .colormap(visual_info.color_map)
                .border_pixel(0),
        )?;

        let mut created_window = Self {
            window_id,
            visual_id: visual_info.visual_id,
            dpi_scale_factor: window_info.scale(),
            _graphics_context,
            #[cfg(feature = "opengl")]
            gl_context: None,
        };

        created_window.set_title(connection, &options.title)?;

        // Register protocols
        connection.conn.change_property32(
            PropMode::REPLACE,
            window_id,
            connection.atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[connection.atoms.WM_DELETE_WINDOW],
        )?;

        connection.conn.flush()?;

        #[cfg(feature = "opengl")]
        if let Some(config) = visual_info.fb_config {
            created_window.create_gl_context(connection, config);
            connection.conn.flush()?;
        }

        Ok(created_window)
    }

    pub fn set_title(&self, connection: &XcbConnection, title: &str) -> Result<(), Box<dyn Error>> {
        connection.conn.change_property8(
            PropMode::REPLACE,
            self.window_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;
        Ok(())
    }

    pub fn show(&self, connection: &XcbConnection) -> Result<(), Box<dyn Error>> {
        connection.conn.map_window(self.window_id)?;
        connection.conn.flush()?;
        Ok(())
    }

    pub fn set_mouse_cursor(&self, connection: &XcbConnection, mouse_cursor: MouseCursor) {
        let xid = connection.get_cursor(mouse_cursor).unwrap();

        if xid != 0 {
            let _ = connection.conn.change_window_attributes(
                self.window_id,
                &ChangeWindowAttributesAux::new().cursor(xid),
            );
            let _ = connection.conn.flush();
        }
    }

    pub fn resize(&self, connection: &XcbConnection, size: Size) {
        let new_window_info = WindowInfo::from_logical_size(size, self.dpi_scale_factor);

        let _ = connection.conn.configure_window(
            self.window_id,
            &ConfigureWindowAux::new()
                .width(new_window_info.physical_size().width)
                .height(new_window_info.physical_size().height),
        );
        let _ = connection.conn.flush();

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it
    }

    pub fn raw_window_handle(&self) -> XcbWindowHandle {
        let mut handle = XcbWindowHandle::empty();

        handle.window = self.window_id;
        handle.visual_id = self.visual_id;

        handle
    }
}

fn create_graphics_context(
    connection: &XcbConnection, parent: Drawable,
) -> Result<Gcontext, Box<dyn Error>> {
    let context_id = connection.conn.generate_id()?;
    let screen = connection.screen();

    connection.conn.create_gc(
        context_id,
        parent,
        &CreateGCAux::new().foreground(screen.black_pixel).graphics_exposures(0),
    )?;

    Ok(context_id)
}

// OpenGL stuff
#[cfg(feature = "opengl")]
const _: () = {
    use crate::gl::platform::GlContext;
    use std::rc::{Rc, Weak};

    use std::ffi::c_ulong;

    impl X11Window {
        // TODO: These APIs could use a couple tweaks now that everything is internal and there is
        //       no error handling anymore at this point. Everything is more or less unchanged
        //       compared to when raw-gl-context was a separate crate.
        #[cfg(feature = "opengl")]
        fn create_gl_context(
            &mut self, connection: &XcbConnection, config: crate::gl::x11::FbConfig,
        ) {
            let window = self.window_id as c_ulong;
            let display = connection.dpy;

            // Because of the visual negotiation we had to take some extra steps to create this context
            let context = unsafe { GlContext::create(window, display, config) }
                .expect("Could not create OpenGL context");

            self.gl_context = Some(Rc::new(context))
        }

        pub fn gl_context(&self) -> Option<Weak<GlContext>> {
            self.gl_context.as_ref().map(Rc::downgrade)
        }
    }
};
