use crate::platform::x11::event_loop::EventLoop;
use crate::platform::x11::visibility_tree::AncestorVisibilityState;
use crate::platform::x11::visual_info::WindowVisualConfig;
use crate::platform::x11::window_thread::WindowThreadShared;
use crate::platform::x11::xcb_connection::get_size_hints;
use crate::platform::x11::xcb_window::XcbWindow;
use crate::platform::*;
use crate::utils::SizingStrategy;
use crate::{warn, MouseCursor, WindowHandler, WindowSettings, WindowSize};
use calloop::LoopSignal;
use dpi::{PhysicalSize, Size};
use raw_window_handle::{DisplayHandle, XlibWindowHandle};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use x11rb::protocol::xproto;
use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt, InputFocus, Visualid};
use x11rb::CURRENT_TIME;

pub struct ScalingFactor {
    system: Cell<Option<f64>>,
    suggested: Cell<Option<f64>>,
}

impl ScalingFactor {
    pub fn get(&self) -> f64 {
        if let Some(factor) = self.system.get() {
            return factor;
        };

        if let Some(factor) = self.suggested.get() {
            return factor;
        }

        1.0
    }

    pub fn suggest(&self, value: f64) -> bool {
        self.suggested.set(Some(value));

        self.system.get().is_none()
    }
}

pub(crate) struct WindowInner {
    // GlContext should be dropped **before** XcbConnection is dropped
    #[cfg(feature = "opengl")]
    gl_context: Option<super::gl::GlContext>,

    pub(crate) xcb_window: XcbWindow,
    pub(crate) connection: Rc<X11Connection>,

    pub(crate) scaling_factor: ScalingFactor,

    window_size: Cell<PhysicalSize<u16>>,
    pub(crate) sizing_strategy: SizingStrategy,
    mouse_cursor: Cell<MouseCursor>,
    pub(crate) visual_id: Visualid,

    pub(crate) is_focused: Cell<bool>,
    pub(crate) is_mapped: Cell<bool>,
    pub(crate) loop_signal: LoopSignal,

    pub(crate) visibility_state: AncestorVisibilityState,

    pub(crate) main_thread_shared: Arc<WindowThreadShared>,
}

impl WindowInner {
    pub(crate) fn create(
        options: WindowSettings, ev_loop: &calloop::EventLoop<'static, EventLoop>,
        shared: Arc<WindowThreadShared>,
    ) -> Result<Rc<Self>> {
        // Connect to the X server
        let xcb_connection = X11Connection::new()?;

        let scaling = xcb_connection.get_scaling();

        let initial_scale_factor = scaling.unwrap_or(1.0);
        shared.set_scaling_factor(initial_scale_factor);

        let physical_size = options.size.to_physical(initial_scale_factor);

        let sizing_strategy = SizingStrategy::from_settings(&options);

        let size_hints = get_size_hints(&sizing_strategy, physical_size, initial_scale_factor);

        #[cfg(feature = "opengl")]
        let visual_info =
            WindowVisualConfig::find_best_visual_config_for_gl(&xcb_connection, options.gl_config)?;

        #[cfg(not(feature = "opengl"))]
        let visual_info = WindowVisualConfig::find_best_visual_config(&xcb_connection)?;

        let connection = Rc::new(xcb_connection);

        let will_have_parent = options.parent.is_some() || options.wait_for_parent;

        let xcb_window = XcbWindow::new(
            Rc::clone(&connection),
            physical_size,
            &visual_info,
            options.parent.map(|p| p.inner.window_id),
        )?;

        if will_have_parent {
            connection.register_tree_structure_events()?.check()?;
        }

        let visibility_state =
            AncestorVisibilityState::discover(&connection, xcb_window.id(), will_have_parent)?;

        let cookies = [
            xcb_window.set_title(&options.title)?,
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
                Some(super::gl::GlContextInner::create(
                    &xcb_window,
                    Rc::clone(&connection),
                    fb_config,
                )?)
            }
        };

        Ok(Rc::new(Self {
            connection,
            xcb_window,
            visual_id: visual_info.visual_id,
            window_size: physical_size.into(),
            scaling_factor: ScalingFactor {
                system: scaling.into(),
                suggested: options.fallback_scale_factor.into(),
            },
            sizing_strategy,
            mouse_cursor: MouseCursor::default().into(),
            loop_signal: ev_loop.get_signal(),

            is_focused: false.into(),
            is_mapped: false.into(),
            main_thread_shared: shared,

            visibility_state,

            #[cfg(feature = "opengl")]
            gl_context,
        }))
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

    pub fn store_size(&self, size: PhysicalSize<u16>) -> PhysicalSize<u16> {
        let previous = self.window_size.replace(size);

        if previous != size {
            self.main_thread_shared.set_size(size);
        }

        previous
    }

    pub fn get_size(&self) -> PhysicalSize<u16> {
        self.window_size.get()
    }

    pub fn request_close(&self) {
        self.loop_signal.stop();
        self.loop_signal.wakeup();
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
        let new_physical_size = size.to_physical(self.scaling_factor.get());
        self.xcb_window.resize(new_physical_size)?.check()?;

        if !self.sizing_strategy.is_resizable() {
            let size_hints =
                get_size_hints(&self.sizing_strategy, new_physical_size, self.scale_factor());
            self.xcb_window.set_size_hints(size_hints)?.check()?;
        }

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it

        Ok(())
    }

    pub fn resize_immediately(
        &self, new_size: PhysicalSize<u16>, handler: &dyn WindowHandler,
    ) -> Result<()> {
        let previous = self.store_size(new_size);

        if previous == new_size {
            return Ok(());
        };

        if let Err(e) =
            handler.resized(WindowSize::from_physical(new_size.cast(), self.scaling_factor.get()))
        {
            warn!("Window Handler failed to resize: {}. Reverting to previous size", &e);
            self.store_size(previous);
            return Err(e.into());
        }

        self.xcb_window.resize(new_size.cast())?.check()?; // Will not call handler, as size is the same as above.
        if !self.sizing_strategy.is_resizable() {
            let size_hints = get_size_hints(&self.sizing_strategy, new_size, self.scale_factor());
            self.xcb_window.set_size_hints(size_hints)?.check()?;
        }

        // These come from the Host, no need to notify it about the new size

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

    pub fn raw_id(&self) -> xproto::Window {
        self.xcb_window.id().get()
    }
}
