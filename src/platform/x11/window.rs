use std::error::Error;
use std::num::NonZero;
use std::rc::Rc;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

use super::visual_info::WindowVisualConfig;
use super::X11Connection;
use crate::handler::WindowHandlerBuilder;
use crate::platform::x11::window_shared::WindowInner;
use crate::platform::x11::window_thread::WindowThreadHandle;
use crate::WindowBuilder;

pub struct Window {
    thread: WindowThreadHandle,
}

impl Window {
    pub fn create_window(builder: WindowBuilder, handler: WindowHandlerBuilder) -> Self {
        Self { thread: WindowThreadHandle::start(builder, handler).unwrap() }
    }

    pub fn is_open(&self) -> bool {
        todo!()
    }

    pub fn run_until_closed(self) -> Result<(), Box<dyn Error>> {
        self.thread.join();

        Ok(())
    }
}

pub fn create_window_inner(
    options: WindowBuilder, xcb_connection: Rc<X11Connection>,
) -> Result<Rc<WindowInner>, Box<dyn Error>> {
    // Get screen information
    let screen = xcb_connection.screen();
    let parent_id = options.parent.map(|p| p.window_id).unwrap_or(screen.root);

    let gc_id = xcb_connection.conn.generate_id()?;
    xcb_connection.conn.create_gc(
        gc_id,
        parent_id,
        &CreateGCAux::new().foreground(screen.black_pixel).graphics_exposures(0),
    )?;

    let scaling = xcb_connection.get_scaling();
    let physical_size = options.size.to_physical(scaling);

    #[cfg(feature = "opengl")]
    let visual_info =
        WindowVisualConfig::find_best_visual_config_for_gl(&xcb_connection, options.gl_config)?;

    #[cfg(not(feature = "opengl"))]
    let visual_info = WindowVisualConfig::find_best_visual_config(&xcb_connection)?;

    let Some(window_id) = NonZero::new(xcb_connection.conn.generate_id()?) else {
        unreachable!();
    };

    xcb_connection.conn.create_window(
        visual_info.visual_depth,
        window_id.get(),
        parent_id,
        0,                    // x coordinate of the new window
        0,                    // y coordinate of the new window
        physical_size.width,  // window width
        physical_size.height, // window height
        0,                    // window border
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
                    | EventMask::LEAVE_WINDOW
                    | EventMask::FOCUS_CHANGE,
            )
            // As mentioned above, these two values are needed to be able to create a window
            // with a depth of 32-bits when the parent window has a different depth
            .colormap(visual_info.color_map)
            .border_pixel(0),
    )?;
    xcb_connection.conn.map_window(window_id.get())?;

    // Change window title
    if let Some(title) = options.title {
        xcb_connection.conn.change_property8(
            PropMode::REPLACE,
            window_id.get(),
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;
    }

    xcb_connection.conn.change_property32(
        PropMode::REPLACE,
        window_id.get(),
        xcb_connection.atoms.WM_PROTOCOLS,
        AtomEnum::ATOM,
        &[xcb_connection.atoms.WM_DELETE_WINDOW],
    )?;

    // Enable drag and drop (TODO: Make this toggleable?)
    xcb_connection.conn.change_property32(
        PropMode::REPLACE,
        window_id.get(),
        xcb_connection.atoms.XdndAware,
        AtomEnum::ATOM,
        &[5u32], // Latest version; hasn't changed since 2002
    )?;

    xcb_connection.conn.flush()?;

    #[cfg(feature = "opengl")]
    let gl_context = visual_info.fb_config.map(|fb_config| {
        use std::ffi::c_ulong;

        let window = window_id.get() as c_ulong;

        // Because of the visual negotation we had to take some extra steps to create this context
        let context =
            super::gl::GlContextInner::create(window, Rc::clone(&xcb_connection), fb_config)
                .expect("Could not create OpenGL context");

        Rc::new(context)
    });

    Ok(Rc::new(WindowInner::new(
        xcb_connection,
        window_id,
        physical_size,
        scaling,
        visual_info.visual_id.try_into()?,
        #[cfg(feature = "opengl")]
        gl_context,
    )))
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
