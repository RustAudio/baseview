use crate::platform::x11::visual_info::WindowVisualConfig;
use crate::platform::X11Connection;
use dpi::PhysicalSize;
use std::num::{NonZero, NonZeroU32};
use std::rc::Rc;
use x11rb::connection::Connection;
use x11rb::cookie::VoidCookie;
use x11rb::errors::{ConnectionError, ReplyOrIdError};
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

pub struct XcbWindow {
    connection: Rc<X11Connection>,
    window_id: NonZeroU32,
}

impl XcbWindow {
    pub fn new(
        connection: Rc<X11Connection>, size: PhysicalSize<u16>, visual_info: &WindowVisualConfig,
        parent_id: Option<NonZeroU32>,
    ) -> Result<Self, ReplyOrIdError> {
        let Some(window_id) = NonZero::new(connection.conn.generate_id()?) else {
            unreachable!();
        };

        connection.conn.create_window(
            visual_info.visual_depth,
            window_id.get(),
            parent_id.map_or(connection.screen().root, NonZeroU32::get),
            0,           // x coordinate of the new window
            0,           // y coordinate of the new window
            size.width,  // window width
            size.height, // window height
            0,           // window border
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

        Ok(Self { window_id, connection })
    }

    pub fn map_window(&self) -> Result<VoidCookie<'_, XCBConnection>, ReplyOrIdError> {
        Ok(self.connection.conn.map_window(self.window_id.get())?)
    }

    pub fn resize(
        &self, size: PhysicalSize<u32>,
    ) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.configure_window(
            self.id().get(),
            &ConfigureWindowAux::new().width(size.width).height(size.height),
        )
    }

    pub fn set_title(&self, title: &str) -> Result<VoidCookie<'_, XCBConnection>, ReplyOrIdError> {
        Ok(self.connection.conn.change_property8(
            PropMode::REPLACE,
            self.window_id.get(),
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?)
    }

    pub fn enable_wm_protocols(&self) -> Result<VoidCookie<'_, XCBConnection>, ReplyOrIdError> {
        Ok(self.connection.conn.change_property32(
            PropMode::REPLACE,
            self.window_id.get(),
            self.connection.atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[self.connection.atoms.WM_DELETE_WINDOW],
        )?)
    }

    pub fn enable_dnd_protocols(&self) -> Result<VoidCookie<'_, XCBConnection>, ReplyOrIdError> {
        Ok(self.connection.conn.change_property32(
            PropMode::REPLACE,
            self.window_id.get(),
            self.connection.atoms.XdndAware,
            AtomEnum::ATOM,
            &[5u32], // Latest version; hasn't changed since 2002
        )?)
    }

    #[inline]
    pub fn id(&self) -> NonZeroU32 {
        self.window_id
    }
}

impl Drop for XcbWindow {
    fn drop(&mut self) {
        // TODO: log error
        let Ok(cookie) = self.connection.conn.destroy_window(self.window_id.get()) else { return };
        let _ = cookie.check();
    }
}
