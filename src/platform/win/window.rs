use windows_core::{ComObject, HSTRING};
use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    UI::{Controls::WM_MOUSELEAVE, WindowsAndMessaging::*},
};

use crate::dpi::{PhysicalPosition, PhysicalSize, Size};
use crate::{warn, DamageArea, EventStatus, WindowHandler};
use std::cell::{Cell, OnceCell};

use super::drop_target::DropTarget;
use super::*;
use crate::handler::WindowHandlerBuilder;
use crate::host::Host;
use crate::platform::win::window_state::{RedrawTimer, WindowSharedState, WindowState};
use crate::platform::PlatformError;
use crate::utils::SizingStrategy;
use crate::window::WindowInitializer;
use crate::wrappers::win32::cursor::SystemCursor;
use crate::wrappers::win32::window::*;
use crate::wrappers::win32::{
    ole_initialize, run_thread_message_loop_until, Dpi, DpiAwarenessGuard, LibraryModule, TimerId,
    TimerType, WindowStyle,
};
use crate::{Event, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowSize};

mod mouse;
mod sizing;

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

        hwnd.set_parent(&new_parent.handle)?;

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

pub struct BaseviewWindow {
    window_state: Rc<WindowState>,
    shared_state: Rc<WindowSharedState>,
    initial_size: Size,

    handler_builder: Cell<Option<WindowHandlerBuilder>>,
    handler: OnceCell<Box<dyn WindowHandler>>,
    host: Host,

    // Workaround some VST3 hosts (e.g. Bitwig) not allowing request_resize before show() occurs
    pub host_needs_new_size_notified_on_show: Cell<Option<PhysicalSize<u32>>>,

    // Things not directly used, but kept so their Drop impl runs when the window is destroyed
    _keyboard_hook: Cell<Option<hook::KeyboardHookHandle>>,
    _drop_target: Cell<Option<ComObject<DropTarget>>>,

    #[cfg(feature = "opengl")]
    gl_config: Option<crate::gl::GlConfig>,
}

impl BaseviewWindow {
    pub fn create(shared_state: Rc<WindowSharedState>, init: WindowInitializer) -> Result<HWnd> {
        shared_state.init(&init);

        let style = WindowStyle::from_settings(&init.settings);
        let parent = init.settings.parent.map(|p| p.inner.handle);

        let dpi_ctx =
            DpiAwarenessGuard::new(&shared_state.user32, shared_state.dpi_scaling_strategy.get())?;

        let window_size = shared_state.current_size.get();

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

        let rect = dpi_ctx.client_area_to_nc_area(window_size.into(), style, None)?;
        let title = HSTRING::from(init.settings.title);
        let window = create_window(&title, style, rect.size(), parent, &dpi_ctx, initializer)?;

        Ok(window)
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

    unsafe fn handle_message(
        &self, window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM,
    ) -> Option<LRESULT> {
        unsafe { wnd_proc_inner(window, msg, wparam, lparam, self) }
    }

    fn before_destroy(&self, window: HWnd) {
        let _ = window.revoke_drag_drop();
        // No need to destroy timers: they are all destroyed when the window is invalidated.
    }
}

/// Our custom `wnd_proc` handler. If the result contains a value, then this is returned after
/// handling any deferred tasks. otherwise the default window procedure is invoked.
#[allow(clippy::unwrap_used, reason = "Refactor this in a later PR")] // TODO
unsafe fn wnd_proc_inner(
    window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM, window_bv: &BaseviewWindow,
) -> Option<LRESULT> {
    let window_state = &window_bv.window_state;
    match msg {
        WM_PAINT => {
            if let Some(rect) = window.get_update_rect() {
                window_bv.notify_damage(DamageArea::Rect(rect.into()));
                window_bv.handle_draw();
                window.validate_rect(rect);
            }

            Some(0)
        }
        WM_TIMER => {
            let timer_id = TimerId::from_raw(wparam)?;

            if timer_id == RedrawTimer::ID {
                if !window_state.redraw_timer.is_running() {
                    return None;
                };
                window.invalidate_window();
                return Some(0);
            }

            None
        }
        WM_CLOSE => {
            window_bv.handle_event(Event::Window(WindowEvent::WillClose));

            None
        }
        WM_CHAR | WM_SYSCHAR | WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
        | WM_INPUTLANGCHANGE => {
            let opt_event = window_state.keyboard_state.borrow_mut().process_message(
                window.as_raw(),
                msg,
                wparam,
                lparam,
            );

            if let Some(event) = opt_event {
                window_bv.handle_event(Event::Keyboard(event));
            }

            if msg != WM_SYSKEYDOWN {
                Some(0)
            } else {
                None
            }
        }
        WM_SETFOCUS => {
            window_bv.handle_event(Event::Window(WindowEvent::Focused));

            None
        }
        WM_KILLFOCUS => {
            window_bv.handle_event(Event::Window(WindowEvent::Unfocused));

            None
        }
        WM_SHOWWINDOW => {
            window_bv.handle_new_size_on_show();
            None
        }
        // If WM_SETCURSOR returns `None`, WM_SETCURSOR continues to get handled by the outer window(s),
        // If it returns `Some(1)`, the current window decides what the cursor is
        WM_SETCURSOR => {
            let low_word = lo_word(lparam) as u32;
            let mouse_in_window = low_word == HTCLIENT;
            if mouse_in_window {
                // Here we need to set the cursor back to what the state says, since it can have changed when outside the window
                if let Ok(cursor) = SystemCursor::load(window_state.cursor_icon.get()) {
                    cursor.set()
                }
                Some(1)
            } else {
                // Cursor is being changed by some other window, e.g. when having mouse on the borders to resize it
                None
            }
        }
        // NOTE: `WM_NCDESTROY` is handled in the outer function because this deallocates the window
        //        state
        BV_WINDOW_MUST_CLOSE => {
            let _ = window.destroy();
            Some(0)
        }

        BV_REQUEST_REDRAW => {
            window_state.request_redraw();
            Some(0)
        }

        BV_REQUEST_POLL => {
            window_bv.handle_poll();
            Some(0)
        }
        _ => None,
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
