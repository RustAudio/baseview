use crate::platform::x11::error::CookieExt;
use crate::platform::x11::visual_info::WindowVisualConfig;
use crate::platform::X11Connection;
use dpi::PhysicalSize;
use std::num::{NonZero, NonZeroU32};
use std::rc::Rc;
use x11rb::connection::Connection;
use x11rb::cookie::VoidCookie;
use x11rb::errors::{ConnectionError, ReplyOrIdError};
use x11rb::properties::WmSizeHints;
use x11rb::protocol::present;
use x11rb::protocol::present::ConnectionExt;
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

pub struct XcbWindow {
    connection: Rc<X11Connection>,
    window_id: NonZeroU32,
    present_notify_event_id: Option<NonZeroU32>,
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

        let present_notify_event_id = if !connection.present_supported {
            None
        } else {
            let Some(event_id) = NonZero::new(connection.conn.generate_id()?) else {
                unreachable!();
            };

            Some(event_id)
        };

        Ok(Self { window_id, connection, present_notify_event_id })
    }

    pub fn present_select_input(
        &self,
    ) -> Result<Option<VoidCookie<'_, XCBConnection>>, ConnectionError> {
        let Some(event_id) = self.present_notify_event_id else {
            return Ok(None);
        };

        Ok(Some(self.connection.conn.present_select_input(
            event_id.get(),
            self.window_id.get(),
            present::EventMask::COMPLETE_NOTIFY,
        )?))
    }

    pub fn map_window(&self) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.map_window(self.window_id.get())
    }

    pub fn unmap_window(&self) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.unmap_window(self.window_id.get())
    }

    pub fn resize(
        &self, size: PhysicalSize<u32>,
    ) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.configure_window(
            self.id().get(),
            &ConfigureWindowAux::new().width(size.width).height(size.height),
        )
    }

    pub fn reparent(
        &self, parent: Option<NonZeroU32>,
    ) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.reparent_window(
            self.id().get(),
            parent.map(|i| i.get()).unwrap_or(0),
            0,
            0,
        )
    }

    pub fn set_title(&self, title: &str) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.change_property8(
            PropMode::REPLACE,
            self.window_id.get(),
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )
    }

    pub fn enable_wm_protocols(&self) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.change_property32(
            PropMode::REPLACE,
            self.window_id.get(),
            self.connection.atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[self.connection.atoms.WM_DELETE_WINDOW],
        )
    }

    pub fn enable_dnd_protocols(&self) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        self.connection.conn.change_property32(
            PropMode::REPLACE,
            self.window_id.get(),
            self.connection.atoms.XdndAware,
            AtomEnum::ATOM,
            &[5u32], // Latest version; hasn't changed since 2002
        )
    }

    pub fn set_size_hints(
        &self, size_hints: WmSizeHints,
    ) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        size_hints.set_normal_hints(&self.connection.conn as &XCBConnection, self.window_id.get())
    }

    pub fn present_supported(&self) -> bool {
        self.present_notify_event_id.is_some()
    }

    pub fn present_notify(
        &self, target_msc: u64,
    ) -> Result<VoidCookie<'_, XCBConnection>, ConnectionError> {
        //dbg!(target_msc);
        self.connection.conn.present_notify_msc(self.window_id.get(), 0, target_msc, 1, 0)
    }

    #[inline]
    pub fn id(&self) -> NonZeroU32 {
        self.window_id
    }
}

impl Drop for XcbWindow {
    fn drop(&mut self) {
        if let Some(event_id) = self.present_notify_event_id {
            match self.connection.conn.present_select_input(
                event_id.get(),
                self.window_id.get(),
                present::EventMask::NO_EVENT,
            ) {
                Err(e) => crate::warn!("Failed to send request to switch XPresent off: {}", e),
                Ok(cookie) => cookie.check_warn(),
            }
        }

        match self.connection.conn.destroy_window(self.window_id.get()) {
            Err(e) => crate::warn!("Failed to send request to destroy X window: {}", e),
            Ok(cookie) => cookie.check_warn(),
        }
    }
}
