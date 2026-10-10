use super::prelude::*;
use crate::platform::x11::host_handle::HostHandle;
use raw_window_handle::{DisplayHandle, XlibWindowHandle};
use std::time::Duration;
use x11rb::protocol::xproto;
use x11rb::protocol::xproto::Visualid;

/// Data that is shared between the event loop and the window handler.
pub struct WindowShared {
    pub xcb_window: XcbWindow,
    pub connection: Rc<X11Connection>,
    host: Rc<HostHandle>,
    visual_id: Visualid,

    pub is_focused: Cell<bool>,
    pub stop_own_event_loop: Cell<bool>,

    pub cursor_state: CursorStateShared,
    pub present_state: PresentStateShared,
    pub sizing_state: SizingStateShared,
    pub timer_manager: TimerManager,
    pub redraw_delayed_timers: TimerManager,

    #[cfg(feature = "opengl")]
    gl_context: Option<PlatformGlContext>,
}

impl WindowShared {
    pub(crate) fn create(
        mut settings: WindowSettings, host: Rc<HostHandle>,
    ) -> PlatformResult<Rc<Self>> {
        let connection = X11Connection::connect()?;

        let sizing_state = SizingStateShared::load(&connection, &settings)?;

        let connection = Rc::new(connection);

        let visual_config =
            WindowVisualConfig::find_best_visual_config(&connection, &mut settings)?;

        let parent_id = settings.parent.map(|p| p.inner.window_id);

        let xcb_window =
            XcbWindow::new(Rc::clone(&connection), sizing_state.size(), &visual_config, parent_id)?;

        let cookies = [
            xcb_window.set_title(&settings.title)?,
            xcb_window.enable_wm_protocols()?,
            xcb_window.enable_dnd_protocols()?,
            xcb_window.set_size_hints(sizing_state.make_size_hints())?,
        ];

        for cookie in cookies {
            cookie.check()?;
        }

        Ok(Rc::new(Self {
            visual_id: visual_config.visual_id,

            #[cfg(feature = "opengl")]
            gl_context: visual_config.make_gl_context(&xcb_window, &connection)?,

            is_focused: false.into(),
            stop_own_event_loop: false.into(),

            cursor_state: CursorStateShared::new(),
            present_state: PresentStateShared::new(),
            sizing_state,
            timer_manager: TimerManager::new(&host),
            redraw_delayed_timers: TimerManager::new(&host),
            host,

            xcb_window,
            connection,
        }))
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) -> PlatformResult<()> {
        self.cursor_state.set_mouse_cursor(mouse_cursor, &self.xcb_window)
    }

    pub fn request_close(&self) {
        self.stop_own_event_loop.set(true);
        // TODO: external event loop
    }

    pub fn request_redraw(&self) {
        self.present_state.request_present_notify()
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        self.present_state.request_present_notify_after(
            duration,
            &self.host,
            &self.redraw_delayed_timers,
        );
    }

    pub fn waker(&self) -> WindowWaker {
        todo!()
        /*
        WindowWaker {
            loop_signal: self.loop_signal.clone(),
            shared: Arc::clone(&self.main_thread_shared),
        }*/
    }

    pub fn has_focus(&self) -> bool {
        self.is_focused.get()
    }

    pub fn focus(&self) -> PlatformResult<()> {
        self.xcb_window.focus()
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
    pub fn create_timer(&self, duration: Duration) -> PlatformResult<TimerHandle> {
        self.timer_manager.create_timer(duration, &self.host).map_err(|e| PlatformError::Host(e))
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
