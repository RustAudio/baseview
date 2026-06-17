use crate::gl::GlContext;
use crate::platform::XcbConnection;
use crate::{MouseCursor, Size, WindowInfo};
use std::cell::Cell;
use std::rc::Rc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, InputFocus, Visualid,
    Window as XWindow,
};
use x11rb::CURRENT_TIME;

pub(crate) struct WindowInner {
    // GlContext should be dropped **before** XcbConnection is dropped
    #[cfg(feature = "opengl")]
    gl_context: Option<GlContext>,

    xcb_connection: Rc<XcbConnection>,
    window_id: XWindow,
    window_info: WindowInfo,
    visual_id: Visualid,
    mouse_cursor: Cell<MouseCursor>,

    close_requested: Cell<bool>,
    is_focused: Cell<bool>,
}

impl WindowInner {
    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        if self.mouse_cursor.get() == mouse_cursor {
            return;
        }

        let xid = self.xcb_connection.get_cursor(mouse_cursor).unwrap();

        if xid != 0 {
            let _ = self.xcb_connection.conn.change_window_attributes(
                self.window_id,
                &ChangeWindowAttributesAux::new().cursor(xid),
            );
            let _ = self.xcb_connection.conn.flush();
        }

        self.mouse_cursor.set(mouse_cursor);
    }

    pub fn close(&self) {
        self.close_requested.set(true);
    }

    pub fn has_focus(&self) -> bool {
        self.is_focused.get()
    }

    pub fn focus(&self) {
        let _ = self.xcb_connection.conn.set_input_focus(
            InputFocus::POINTER_ROOT,
            self.window_id,
            CURRENT_TIME,
        );
        let _ = self.xcb_connection.conn.flush();
    }

    pub fn resize(&self, size: Size) {
        let scaling = self.window_info.scale();
        let new_window_info = WindowInfo::from_logical_size(size, scaling);

        let _ = self.xcb_connection.conn.configure_window(
            self.window_id,
            &ConfigureWindowAux::new()
                .width(new_window_info.physical_size().width)
                .height(new_window_info.physical_size().height),
        );
        let _ = self.xcb_connection.conn.flush();

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it
    }
}
