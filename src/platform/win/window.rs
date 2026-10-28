use windows_core::{ComObject, HSTRING};

use crate::dpi::Size;
use crate::{warn, DamageArea, EventStatus};
use std::cell::{Cell, OnceCell};

use super::drop_target::DropTarget;
use super::*;
use crate::platform::win::window_state::{WindowSharedState, WindowState};
use crate::platform::PlatformError;
use crate::utils::SizingStrategy;
use crate::window::WindowInitializer;
use crate::wrappers::win32::window::*;
use crate::wrappers::win32::{
    ole_initialize, run_thread_message_loop_until, Dpi, DpiAwarenessGuard, LibraryModule, TimerId,
    WindowStyle,
};
use crate::{Event, WindowEvent, WindowSize};

pub struct WindowHandle {
    init: Cell<Option<Box<WindowInitializer>>>,
    hwnd: Cell<Option<HWnd>>,
    state: Rc<WindowSharedState>,
}

impl WindowHandle {
    pub fn run_until_closed(self) -> Result<()> {
        self.show()?;

        run_thread_message_loop_until(|| !self.is_open())?;
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.state.is_alive.get()
    }

    pub fn size(&self) -> WindowSize {
        self.state.size()
    }

    pub fn resize(&self, new_size: Size) -> Result<()> {
        let new_size = self.state.sizing_strategy.adjust_size(new_size, self.size()).physical;

        if new_size == self.state.current_size.get() {
            return Ok(());
        }

        let Some(hwnd) = self.hwnd.get() else {
            self.state.current_size.set(new_size);
            return Ok(());
        };

        let _guard = self.state.originate_host_resize();
        let dpi_ctx =
            DpiAwarenessGuard::new(&self.state.user32, self.state.dpi_scaling_strategy.get())?;
        hwnd.resize_and_activate(new_size, self.state.current_dpi.get(), &dpi_ctx)?;

        if self.state.current_size.get() == new_size {
            Ok(())
        } else {
            Err(PlatformError::UnexpectedResizeResult {
                new_actual_size: self.state.current_size.get(),
            })
        }
    }

    #[inline]
    pub fn sizing_strategy(&self) -> SizingStrategy {
        self.state.sizing_strategy
    }

    pub fn suggest_scale_factor(&self, new_host_scale_factor: f64) -> Result<()> {
        let new_dpi = Dpi::from_scale_factor(new_host_scale_factor)
            .ok_or(PlatformError::InvalidScaleFactor)?;

        let current_scale_factor = self.state.scale_factor();
        self.state.fallback_scale_factor.set(Some(new_host_scale_factor));

        let dpi_scaling_strategy = self.state.dpi_scaling_strategy.get();

        if !dpi_scaling_strategy.should_use_host_suggested_scale_factor {
            return Ok(());
        }

        self.state.current_dpi.set(new_dpi);

        let Some(hwnd) = self.hwnd.get() else { return Ok(()) };

        let current_size = self.state.current_size.get();
        let new_size = self
            .state
            .current_size
            .get()
            .to_logical::<f64>(current_scale_factor)
            .to_physical(new_host_scale_factor);

        // This call doesn't meaningfully change the scaling factor, ignore the result
        if current_size == new_size {
            return Ok(());
        }

        let _guard = self.state.originate_host_resize();
        let dpi_ctx = DpiAwarenessGuard::new(&self.state.user32, dpi_scaling_strategy)?;

        hwnd.resize_and_activate(new_size, new_dpi, &dpi_ctx)?;

        if self.state.current_size.get() == new_size {
            Ok(())
        } else {
            Err(PlatformError::UnexpectedResizeResult {
                new_actual_size: self.state.current_size.get(),
            })
        }
    }

    pub fn set_parent(&self, new_parent: ParentWindowHandle) -> Result<()> {
        let hwnd = match self.hwnd.get() {
            Some(hwnd) => hwnd,
            None => {
                let Some(mut init) = self.init.take() else { return Ok(()) };
                init.settings.parent = Some(new_parent.into());

                let window = BaseviewWindow::create(Rc::clone(&self.state), *init)?;
                self.hwnd.set(Some(window));

                return Ok(());
            }
        };

        if !self.state.parented.get() {
            panic!("Called set_parent on a floating window")
        }

        hwnd.set_parent(&new_parent.hwnd())?;

        Ok(())
    }

    #[inline]
    pub fn handle_main_thread_callback(&self) {
        // No-op
    }

    pub fn show(&self) -> Result<()> {
        let hwnd = match self.hwnd.get() {
            Some(hwnd) => hwnd,
            None => {
                let Some(init) = self.init.take() else { return Ok(()) };

                let window = BaseviewWindow::create(Rc::clone(&self.state), *init)?;
                self.hwnd.set(Some(window));

                window
            }
        };

        hwnd.show_and_activate();

        Ok(())
    }

    pub fn hide(&self) -> Result<()> {
        let Some(hwnd) = self.hwnd.get() else { return Ok(()) };
        hwnd.hide();

        Ok(())
    }

    pub fn waker(&self) -> WindowWaker {
        self.state.window_waker_source.waker()
    }

    pub fn request_poll(&self) -> Result<()> {
        if let Some(hwnd) = self.hwnd.get() {
            hwnd.post_request_poll();
        }

        Ok(())
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        if !self.state.is_alive.get() {
            return;
        }

        if let Some(hwnd) = self.hwnd.take() {
            let _guard = self.state.originate_host_destroy();
            if let Err(e) = hwnd.destroy() {
                warn!("Failed to destroy window: {}", e);
            }
        }
    }
}

impl BaseviewWindow {
    pub fn create(shared_state: Rc<WindowSharedState>, init: WindowInitializer) -> Result<HWnd> {
        shared_state.init(&init);

        let style = WindowStyle::from_settings(&init.settings);
        let parent = init.settings.parent.map(|p| p.inner.hwnd());

        let dpi_ctx =
            DpiAwarenessGuard::new(&shared_state.user32, shared_state.dpi_scaling_strategy.get())?;

        let window_size = shared_state.current_size.get();
        let rect = dpi_ctx.client_area_to_nc_area(window_size.into(), style, None)?;
        let title = HSTRING::from(init.settings.title);

        let initializer = {
            let shared_state = Rc::clone(&shared_state);

            move |hwnd: HWnd| {
                shared_state.set_hwnd(hwnd);

                let window_state = Rc::new(WindowState::new(
                    hwnd,
                    shared_state.user32.clone(),
                    Rc::clone(&shared_state),
                ));

                BaseviewWindow {
                    window_state,
                    initial_size: init.settings.size,
                    handler_builder: Cell::new(Some(init.builder)),
                    handler: OnceCell::new(),
                    shared_state,
                    host: init.host,
                    host_needs_new_size_notified_on_show: None.into(),

                    _drop_target: None.into(),
                    _keyboard_hook: None.into(),

                    #[cfg(feature = "opengl")]
                    gl_config: init.settings.gl_config,
                }
            }
        };
        Ok(create_window(&title, style, rect.size(), parent, &dpi_ctx, initializer)?)
    }

    fn notify_destroyed_to_host(&self) {
        if self.shared_state.destroy_host_originated.get() {
            return;
        };

        self.host.notify_destroyed()
    }

    pub(crate) fn notify_damage(&self, damage: DamageArea) {
        let Some(handler) = self.handler.get() else { return };
        handler.damage(damage);
    }

    pub(crate) fn handle_draw(&self) {
        let Some(handler) = self.handler.get() else { return };

        handler.poll();

        self.window_state.redraw_requested.set(false);
        if let Err(e) = handler.draw() {
            warn!("Error while rendering frame: {}", e);
            self.window_state.request_close();
        }

        self.window_state.setup_redraw_request_for_next_frame();
    }

    pub(crate) fn handle_poll(&self) {
        let Some(handler) = self.handler.get() else { return };

        handler.poll();
    }

    pub(crate) fn handle_event(&self, event: Event) -> EventStatus {
        let Some(handler) = self.handler.get() else {
            return EventStatus::Ignored;
        };

        handler.on_event(event)
    }

    pub(crate) fn handle_delayed_redraw_wm_timer(&self, id: TimerId) {
        match self.shared_state.delayed_redraw_timers.remove_if_exists(id) {
            Ok(false) => {}
            Err(e) => {
                warn!("Could not remove timer: {}", e);
            }
            Ok(true) => {
                self.window_state.request_redraw();
            }
        }
    }

    pub(crate) fn handle_user_wm_timer(&self, id: TimerId) {
        if self.shared_state.user_timers.exists(id) {
            let Some(handler) = self.handler.get() else { return };

            handler.on_timer((&id).into());
        }
    }
}

impl Drop for BaseviewWindow {
    fn drop(&mut self) {
        self.shared_state.is_alive.set(false);
        self.notify_destroyed_to_host();
    }
}

impl WindowImpl for BaseviewWindow {
    fn non_client_create(&self, window: HWnd) -> std::result::Result<(), PlatformError> {
        if self.shared_state.dpi_scaling_strategy.get().should_enable_nc_dpi_scaling_manually {
            window.enable_non_client_dpi_scaling(&self.shared_state.user32);
        }

        Ok(())
    }

    fn after_create(&self, window: HWnd) -> core::result::Result<(), PlatformError> {
        let window_state = &self.window_state;

        self._keyboard_hook.set(Some(hook::init_keyboard_hook(window.as_raw())));

        // Now we can get the actual dpi of the window.
        self.initialize_window_dpi(window)?;

        let drop_target = ComObject::new(DropTarget::new(Rc::downgrade(window_state), window));
        self._drop_target.set(Some(drop_target.clone()));

        ole_initialize()?;
        window.register_drag_drop(drop_target.as_interface())?;

        #[cfg(feature = "opengl")]
        if let Some(gl_config) = self.gl_config.clone() {
            let gl_context = gl::GlContextInner::create(window, gl_config)?;

            let Ok(()) = self.window_state.gl_context.set(Rc::new(gl_context)) else {
                unreachable!();
            };
        };

        let handler = {
            let context = crate::WindowContext::new(Rc::clone(&self.window_state));
            let Some(handler_builder) = self.handler_builder.take() else {
                unreachable!();
            };

            handler_builder.build(context)?
        };
        let Ok(()) = self.handler.set(handler) else { unreachable!() };

        Ok(())
    }

    fn before_destroy(&self, window: HWnd) {
        let _ = window.revoke_drag_drop();
        // No need to destroy timers: they are all destroyed when the window is invalidated.
    }

    fn on_close(&self) {
        self.handle_event(Event::Window(WindowEvent::WillClose));
    }

    fn on_show_window(&self) {
        self.handle_new_size_on_show();
    }

    fn on_close_requested(&self, window: HWnd) {
        let _ = window.destroy();
    }
}

impl WindowHandle {
    pub fn create_window(init: WindowInitializer) -> Result<WindowHandle> {
        let extended_user_32 = LibraryModule::load()?;

        let shared_state = WindowSharedState::new(extended_user_32, &init);

        if init.settings.wait_for_parent && init.settings.parent.is_none() {
            return Ok(WindowHandle {
                hwnd: None.into(),
                state: shared_state,
                init: Some(Box::new(init)).into(),
            });
        }

        let window = BaseviewWindow::create(Rc::clone(&shared_state), init)?;

        Ok(WindowHandle { hwnd: Some(window).into(), state: shared_state, init: None.into() })
    }
}

pub fn copy_to_clipboard(_data: &str) {
    unimplemented!()
}
