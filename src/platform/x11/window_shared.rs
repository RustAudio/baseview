use crate::platform::x11::visual_info::WindowVisualConfig;
use crate::platform::X11Connection;
use crate::{MouseCursor, Size, WindowInfo};
use raw_window_handle::{DisplayHandle, XcbWindowHandle};
use std::cell::Cell;
use std::num::NonZero;
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
    gl_context: Option<crate::gl::GlContext>,

    pub(crate) connection: Rc<X11Connection>,
    pub(crate) window_id: NonZero<XWindow>,
    pub(crate) window_info: Cell<WindowInfo>,
    visual_id: Visualid,
    mouse_cursor: Cell<MouseCursor>,

    pub(crate) close_requested: Cell<bool>,
    pub(crate) is_focused: Cell<bool>,
}

impl WindowInner {
    pub(crate) fn new(
        connection: Rc<X11Connection>, window_id: NonZero<XWindow>, window_info: WindowInfo,
        visual_id: Visualid, #[cfg(feature = "opengl")] gl_context: Option<crate::gl::GlContext>,
    ) -> Self {
        Self {
            connection,
            window_id,
            window_info: window_info.into(),
            visual_id,
            mouse_cursor: MouseCursor::default().into(),

            close_requested: false.into(),
            is_focused: false.into(),

            #[cfg(feature = "opengl")]
            gl_context,
        }
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        if self.mouse_cursor.get() == mouse_cursor {
            return;
        }

        let xid = self.connection.get_cursor(mouse_cursor).unwrap();

        if xid != 0 {
            let _ = self.connection.conn.change_window_attributes(
                self.window_id.get(),
                &ChangeWindowAttributesAux::new().cursor(xid),
            );
            let _ = self.connection.conn.flush();
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
        let _ = self.connection.conn.set_input_focus(
            InputFocus::POINTER_ROOT,
            self.window_id,
            CURRENT_TIME,
        );
        let _ = self.connection.conn.flush();
    }

    pub fn resize(&self, size: Size) {
        let scaling = self.window_info.get().scale();
        let new_window_info = WindowInfo::from_logical_size(size, scaling);

        let _ = self.connection.conn.configure_window(
            self.window_id.get(),
            &ConfigureWindowAux::new()
                .width(new_window_info.physical_size().width)
                .height(new_window_info.physical_size().height),
        );
        let _ = self.connection.conn.flush();

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it
    }

    pub fn window_handle(&self) -> raw_window_handle::WindowHandle<'_> {
        let handle = XcbWindowHandle::new(self.window_id);
        unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) }
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        self.connection.display_handle()
    }
}
