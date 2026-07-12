use crate::platform::x11::xcb_window::XcbWindow;
use crate::platform::*;
use crate::{MouseCursor, WindowSize};
use dpi::{PhysicalSize, Size};
use raw_window_handle::{DisplayHandle, XlibWindowHandle};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, InputFocus, Visualid,
};
use x11rb::CURRENT_TIME;

pub(crate) struct WindowInner {
    // GlContext should be dropped **before** XcbConnection is dropped
    #[cfg(feature = "opengl")]
    gl_context: Option<super::gl::GlContext>,

    pub(crate) xcb_window: XcbWindow,
    pub(crate) connection: Rc<X11Connection>,
    pub(crate) scaling_factor: Cell<f64>,
    pub(crate) window_size: Cell<PhysicalSize<u16>>,
    mouse_cursor: Cell<MouseCursor>,
    pub(crate) visual_id: Visualid,

    pub(crate) close_requested: Cell<bool>,
    pub(crate) is_focused: Cell<bool>,
}

impl WindowInner {
    pub(crate) fn new(
        connection: Rc<X11Connection>, xcb_window: XcbWindow, window_size: PhysicalSize<u16>,
        scale_factor: f64, visual_id: Visualid,
        #[cfg(feature = "opengl")] gl_context: Option<super::gl::GlContext>,
    ) -> Self {
        Self {
            connection,
            xcb_window,
            visual_id,
            window_size: window_size.into(),
            scaling_factor: scale_factor.into(),
            mouse_cursor: MouseCursor::default().into(),

            close_requested: false.into(),
            is_focused: false.into(),

            #[cfg(feature = "opengl")]
            gl_context,
        }
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) -> Result<()> {
        if self.mouse_cursor.get() == mouse_cursor {
            return Ok(());
        }

        let xid = self.connection.get_cursor(mouse_cursor)?;

        if xid != 0 {
            self.connection
                .conn
                .change_window_attributes(
                    self.xcb_window.id().get(),
                    &ChangeWindowAttributesAux::new().cursor(xid),
                )?
                .check()?;
        }

        self.mouse_cursor.set(mouse_cursor);

        Ok(())
    }

    pub fn request_close(&self) {
        self.close_requested.set(true);
    }

    pub fn has_focus(&self) -> bool {
        self.is_focused.get()
    }

    pub fn focus(&self) -> Result<()> {
        self.connection
            .conn
            .set_input_focus(InputFocus::POINTER_ROOT, self.xcb_window.id(), CURRENT_TIME)?
            .check()?;

        Ok(())
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        let new_physical_size = size.to_physical::<u32>(self.scaling_factor.get());

        self.connection
            .conn
            .configure_window(
                self.xcb_window.id().get(),
                &ConfigureWindowAux::new()
                    .width(new_physical_size.width)
                    .height(new_physical_size.height),
            )?
            .check()?;

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it

        Ok(())
    }

    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let mut handle = XlibWindowHandle::new(self.xcb_window.id().get() as _);
        handle.visual_id = self.visual_id.into();
        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        self.connection.conn.xlib_display_handle()
    }

    pub fn platform_handle(&self) -> PlatformHandle {
        PlatformHandle {
            connection: Arc::clone(&self.connection.conn),
            window_id: self.xcb_window.id(),
            visual_id: self.visual_id,
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        Some(crate::gl::GlContext::new(Rc::clone(self.gl_context.as_ref()?)))
    }

    pub fn scale_factor(&self) -> f64 {
        self.scaling_factor.get()
    }

    pub fn size(&self) -> WindowSize {
        WindowSize::from_physical(self.window_size.get().cast(), self.scaling_factor.get())
    }
}
