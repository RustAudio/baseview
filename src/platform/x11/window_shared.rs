use super::prelude::*;
use crate::platform::x11::event_loop::EventLoop;
use crate::platform::x11::present::PresentStateShared;
use crate::platform::x11::sizing::SizingStateShared;
use crate::platform::x11::timer::insert_timer;
use crate::platform::x11::visual_info::WindowVisualConfig;
use crate::platform::x11::waker::WindowWaker;
use crate::platform::x11::window_thread::WindowThreadShared;
use crate::platform::x11::xcb_window::XcbWindow;
use crate::{MouseCursor, WindowSettings, WindowSize};
use calloop::{LoopHandle, LoopSignal};
use dpi::Size;
use raw_window_handle::{DisplayHandle, XlibWindowHandle};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use x11rb::protocol::xproto;
use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt, InputFocus, Visualid};
use x11rb::CURRENT_TIME;

/// Data that is shared between the event loop and the window handler.
pub struct WindowShared {
    #[cfg(feature = "opengl")]
    gl_context: Option<PlatformGlContext>,

    pub xcb_window: XcbWindow,
    pub connection: Rc<X11Connection>,
    visual_id: Visualid,

    mouse_cursor: Cell<MouseCursor>,
    pub poll_requested: Cell<bool>,
    pub is_focused: Cell<bool>,

    pub present_state: PresentStateShared,
    pub sizing_state: SizingStateShared,

    loop_signal: LoopSignal,
    loop_handle: LoopHandle<'static, EventLoop>,

    pub main_thread_shared: Arc<WindowThreadShared>,
}

impl WindowShared {
    pub(crate) fn create(
        settings: WindowSettings, ev_loop: &calloop::EventLoop<'static, EventLoop>,
        thread_shared: Arc<WindowThreadShared>,
    ) -> PlatformResult<Rc<Self>> {
        let connection = X11Connection::connect()?;

        let sizing_state = SizingStateShared::load(&connection, &thread_shared.sizing, &settings)?;
        let size_hints = sizing_state.make_size_hints();

        let connection = Rc::new(connection);

        #[cfg(feature = "opengl")]
        let visual_info =
            WindowVisualConfig::find_best_visual_config_for_gl(&connection, settings.gl_config)?;

        #[cfg(not(feature = "opengl"))]
        let visual_info = WindowVisualConfig::find_best_visual_config(&connection)?;

        settings.parent.is_some() || settings.wait_for_parent;
        let parent_id = settings.parent.map(|p| p.inner.window_id);

        let xcb_window =
            XcbWindow::new(Rc::clone(&connection), sizing_state.size(), &visual_info, parent_id)?;

        let cookies = [
            xcb_window.set_title(&settings.title)?,
            xcb_window.enable_wm_protocols()?,
            xcb_window.enable_dnd_protocols()?,
            xcb_window.set_size_hints(size_hints)?,
        ];

        for cookie in cookies {
            cookie.check()?;
        }

        #[cfg(feature = "opengl")]
        let gl_context = match visual_info.fb_config {
            None => None,
            Some(fb_config) => {
                // Because of the visual negotation we had to take some extra steps to create this context
                Some(GlContextInner::create(&xcb_window, &connection, fb_config)?)
            }
        };

        Ok(Rc::new(Self {
            connection,
            xcb_window,
            visual_id: visual_info.visual_id,
            mouse_cursor: MouseCursor::default().into(),
            loop_signal: ev_loop.get_signal(),
            loop_handle: ev_loop.handle(),

            is_focused: false.into(),
            present_state: PresentStateShared::new(),
            sizing_state,
            poll_requested: false.into(),
            main_thread_shared: thread_shared,

            #[cfg(feature = "opengl")]
            gl_context,
        }))
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) -> PlatformResult<()> {
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
        self.loop_signal.stop();
        self.loop_signal.wakeup();
    }

    pub fn request_redraw(&self) {
        self.present_state.request_present_notify()
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        self.present_state.request_present_notify_after(duration, &self.loop_handle);
    }

    pub fn waker(&self) -> WindowWaker {
        WindowWaker {
            loop_signal: self.loop_signal.clone(),
            shared: Arc::clone(&self.main_thread_shared),
        }
    }

    pub fn has_focus(&self) -> bool {
        self.is_focused.get()
    }

    pub fn focus(&self) -> PlatformResult<()> {
        self.connection
            .conn
            .set_input_focus(InputFocus::POINTER_ROOT, self.xcb_window.id(), CURRENT_TIME)?
            .check()?;

        Ok(())
    }

    pub fn resize(&self, new_size: Size) -> PlatformResult<()> {
        self.sizing_state.resize_from_handler(new_size, &self.xcb_window)
    }

    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let mut handle = XlibWindowHandle::new(self.xcb_window.id().get() as _);
        handle.visual_id = self.visual_id.into();
        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        self.connection.conn.xlib_display_handle()
    }

    pub fn platform_handle(&self) -> super::PlatformHandle {
        super::PlatformHandle {
            connection: Arc::clone(&self.connection.conn),
            window_id: self.xcb_window.id(),
            visual_id: self.visual_id,
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<GlContext> {
        Some(GlContext::new(Rc::clone(self.gl_context.as_ref()?)))
    }
    #[inline]
    pub fn create_timer(&self, duration: Duration) -> Result<TimerHandle> {
        insert_timer(&self.loop_handle, duration)
    }

    pub fn scale_factor(&self) -> f64 {
        self.sizing_state.scale_factor()
    }

    pub fn size(&self) -> WindowSize {
        self.sizing_state.window_size()
    }

    pub fn raw_id(&self) -> xproto::Window {
        self.xcb_window.id().get()
    }
}
