use crate::dpi::{PhysicalSize, Size};
use crate::platform::win::dpi::DpiScalingStrategy;
use crate::platform::win::keyboard::KeyboardState;
use crate::platform::win::waker::WindowWakerSource;
use crate::platform::{PlatformHandle, TimerHandle, WindowWaker};
use crate::utils::SizingStrategy;
use crate::window::WindowInitializer;
use crate::wrappers::win32::cursor::SystemCursor;
use crate::wrappers::win32::h_instance::HInstance;
use crate::wrappers::win32::window::{HWnd, PostMessageExt};
use crate::wrappers::win32::{
    Dpi, DpiAwarenessGuard, ExtendedUser32, LibraryModule, TimerList, TimerSlot,
};
use crate::{MouseCursor, WindowSize};
use raw_window_handle::{DisplayHandle, Win32WindowHandle};
use std::cell::{Cell, Ref, RefCell};
use std::num::NonZeroIsize;
use std::rc::Rc;
use std::time::Duration;

const REDRAW_TIMER_DELAY_MSEC: u32 = 15;

/// All data associated with the window.
pub(crate) struct WindowState {
    /// The HWND belonging to this window.
    pub hwnd: HWnd,
    pub keyboard_state: RefCell<KeyboardState>,
    pub mouse_button_counter: Cell<usize>,
    pub mouse_was_outside_window: Cell<bool>,
    pub cursor_icon: Cell<MouseCursor>,

    pub user32: LibraryModule<ExtendedUser32>,
    pub shared: Rc<WindowSharedState>,
    pub(crate) redraw_timer: TimerSlot,
    pub redraw_requested: Cell<bool>,

    #[cfg(feature = "opengl")]
    pub gl_context: std::cell::OnceCell<super::gl::PlatformGlContext>,
}

impl WindowState {
    pub fn new(
        hwnd: HWnd, user32: LibraryModule<ExtendedUser32>, shared: Rc<WindowSharedState>,
    ) -> Self {
        Self {
            hwnd,
            keyboard_state: RefCell::new(KeyboardState::new()),
            mouse_button_counter: Cell::new(0),
            mouse_was_outside_window: true.into(),
            cursor_icon: Cell::new(MouseCursor::Default),
            redraw_timer: TimerSlot::empty(hwnd),
            user32,
            shared,
            redraw_requested: Cell::new(false),

            #[cfg(feature = "opengl")]
            gl_context: std::cell::OnceCell::new(),
        }
    }

    /// Returns the current size of this window.
    pub fn size(&self) -> WindowSize {
        self.shared.size()
    }

    /// Returns the current scale factor of this window.
    pub fn scale_factor(&self) -> f64 {
        self.shared.scale_factor()
    }

    pub(crate) fn keyboard_state(&self) -> Ref<'_, KeyboardState> {
        self.keyboard_state.borrow()
    }

    pub fn request_close(&self) {
        self.hwnd.post_must_close();
    }

    pub fn has_focus(&self) -> bool {
        HWnd::get_focused_window() == self.hwnd.as_raw()
    }

    pub fn focus(&self) -> Result<(), super::PlatformError> {
        self.hwnd.set_focus()?;
        Ok(())
    }

    pub fn resize(&self, size: Size) -> Result<(), super::PlatformError> {
        // `self.window_info` will be modified in response to the `WM_SIZE` event that
        // follows the `SetWindowPos()` call
        let dpi = self.shared.current_dpi.get();
        let new_size = size.to_physical(self.shared.scale_factor());

        let ctx = DpiAwarenessGuard::new(&self.user32, self.shared.dpi_scaling_strategy.get())?;

        self.hwnd.resize_and_activate(new_size, dpi, &ctx)?;
        Ok(())
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) -> Result<(), super::PlatformError> {
        self.cursor_icon.set(mouse_cursor);
        if let Ok(cursor) = SystemCursor::load(mouse_cursor) {
            cursor.set()
        }

        Ok(())
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        Some(crate::gl::GlContext::new(Rc::clone(self.gl_context.get()?)))
    }

    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let Some(hwnd) = NonZeroIsize::new(self.hwnd.as_raw() as _) else { unreachable!() };
        let mut handle = Win32WindowHandle::new(hwnd);
        handle.hinstance = Some(HInstance::get_from_dll().addr());

        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::windows()
    }

    pub fn platform_handle(&self) -> PlatformHandle {
        let Some(hwnd) = NonZeroIsize::new(self.hwnd.as_raw() as _) else { unreachable!() };
        PlatformHandle { hwnd }
    }

    pub fn request_redraw(&self) {
        self.redraw_requested.set(true);

        self.redraw_timer.start_if_not_running(REDRAW_TIMER_DELAY_MSEC);
    }

    pub fn setup_redraw_request_for_next_frame(&self) {
        let should_redraw_next_frame = self.redraw_requested.take();

        self.redraw_timer.set_running(should_redraw_next_frame, REDRAW_TIMER_DELAY_MSEC);
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        if let Err(e) = self.shared.delayed_redraw_timers.add_new_timer(self.hwnd, duration) {
            crate::warn!("Request Redraw failed: Could not add timer: {}", e)
        }
    }

    #[inline]
    pub fn waker(&self) -> WindowWaker {
        self.shared.window_waker_source.waker()
    }

    #[inline]
    pub fn create_timer(&self, duration: Duration) -> Result<TimerHandle, super::PlatformError> {
        Ok(self.shared.user_timers.add_new_timer(self.hwnd, duration)?)
    }
}

pub struct WindowSharedState {
    pub hwnd: Cell<Option<HWnd>>,
    pub parented: Cell<bool>,
    pub is_alive: Cell<bool>,
    pub current_size: Cell<PhysicalSize<u32>>,
    pub current_dpi: Cell<Dpi>,
    pub fallback_scale_factor: Cell<Option<f64>>,
    pub resize_host_originated: Cell<bool>,
    pub destroy_host_originated: Cell<bool>,
    pub dpi_scaling_strategy: Cell<DpiScalingStrategy>,

    pub user32: LibraryModule<ExtendedUser32>,
    pub sizing_strategy: SizingStrategy,
    pub delayed_redraw_timers: TimerList,
    pub user_timers: TimerList,
    pub window_waker_source: WindowWakerSource,
}

impl WindowSharedState {
    pub fn new(user32: LibraryModule<ExtendedUser32>, init: &WindowInitializer) -> Rc<Self> {
        let state = Self {
            sizing_strategy: SizingStrategy::from_settings(&init.settings),
            user32,

            // Internal state
            is_alive: true.into(),
            resize_host_originated: false.into(),
            destroy_host_originated: false.into(),

            // Overriden by init() below
            parented: false.into(),
            current_dpi: Dpi::default().into(),
            current_size: init.settings.size.to_physical(1.0).into(),
            fallback_scale_factor: init.settings.fallback_scale_factor.into(),
            dpi_scaling_strategy: DpiScalingStrategy::default().into(),
            delayed_redraw_timers: TimerList::new(),
            user_timers: TimerList::new(),
            window_waker_source: WindowWakerSource::new(),
            hwnd: None.into(),
        };

        state.init(init);

        state.into()
    }

    pub fn init(&self, init: &WindowInitializer) {
        let parent = init.settings.parent.as_ref().map(|p| p.inner.handle);
        let strategy = DpiScalingStrategy::get(
            Some(&self.user32),
            parent,
            #[cfg(feature = "opengl")]
            &init.settings,
        );

        if strategy.assume_96_dpi {
            self.current_dpi.set(Dpi::default());
            self.current_size.set(init.settings.size.to_physical(1.0));
            self.fallback_scale_factor.set(Some(1.0));
        } else {
            let parent_dpi = parent.and_then(|p| p.get_dpi(&self.user32));
            let scale_factor = parent_dpi
                .map(|dpi| dpi.scale_factor())
                .or(init.settings.fallback_scale_factor)
                .unwrap_or(1.0);

            self.current_dpi.set(parent_dpi.unwrap_or_default());
            self.current_size.set(init.settings.size.to_physical(scale_factor));
            self.fallback_scale_factor.set(init.settings.fallback_scale_factor);
        }

        self.parented.set(init.settings.parent.is_some() || init.settings.wait_for_parent);
        self.dpi_scaling_strategy.set(strategy);
    }

    pub fn set_hwnd(&self, hwnd: HWnd) {
        self.hwnd.set(Some(hwnd));
        self.window_waker_source.set(hwnd)
    }

    pub fn size(&self) -> WindowSize {
        WindowSize::from_physical(self.current_size.get(), self.scale_factor())
    }

    pub fn scale_factor(&self) -> f64 {
        let strategy = self.dpi_scaling_strategy.get();
        if strategy.assume_96_dpi {
            return 1.0;
        }

        if strategy.should_use_host_suggested_scale_factor {
            return self.fallback_scale_factor.get().unwrap_or(1.0);
        }

        self.current_dpi.get().scale_factor()
    }

    pub fn originate_host_resize(&self) -> impl Drop + use<'_> {
        self.resize_host_originated.set(true);
        Guard(&self.resize_host_originated)
    }

    pub fn originate_host_destroy(&self) -> impl Drop + use<'_> {
        self.destroy_host_originated.set(true);
        Guard(&self.destroy_host_originated)
    }
}

impl Drop for WindowSharedState {
    fn drop(&mut self) {
        if let Some(hwnd) = self.hwnd.get() {
            self.user_timers.stop_and_destroy_all(hwnd);
            self.delayed_redraw_timers.stop_and_destroy_all(hwnd);
        }
    }
}

struct Guard<'a>(&'a Cell<bool>);
impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
