use crate::platform::win::keyboard::KeyboardState;
use crate::wrappers::win32::cursor::SystemCursor;
use crate::wrappers::win32::window::HWnd;
use crate::wrappers::win32::{Dpi, ExtendedUser32};
use crate::{Event, EventStatus, MouseCursor, WindowHandler, WindowScalePolicy, WindowSize};
use dpi::{PhysicalSize, Size};
use raw_window_handle::{DisplayHandle, Win32WindowHandle};
use std::cell::{Cell, OnceCell, Ref, RefCell};
use std::num::NonZeroIsize;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

/// All data associated with the window.
pub(crate) struct WindowState {
    /// The HWND belonging to this window.
    pub hwnd: HWND,
    pub current_size: Cell<PhysicalSize<u32>>,
    pub current_dpi: Cell<Dpi>, // None if in non-system scale policy
    pub keyboard_state: RefCell<KeyboardState>,
    pub mouse_button_counter: Cell<usize>,
    pub mouse_was_outside_window: Cell<bool>,
    pub cursor_icon: Cell<MouseCursor>,
    // Initialized late so the `Window` can hold a reference to this `WindowState`
    pub handler: OnceCell<Box<dyn WindowHandler>>,
    pub scale_policy: WindowScalePolicy,

    pub user32: ExtendedUser32,

    #[cfg(feature = "opengl")]
    pub gl_context: OnceCell<super::gl::GlContext>,
}

impl WindowState {
    pub fn new(
        hwnd: HWND, current_size: PhysicalSize<u32>, scale_policy: WindowScalePolicy,
        user32: ExtendedUser32,
    ) -> Self {
        Self {
            hwnd,
            current_dpi: Dpi::default().into(),
            current_size: current_size.into(),
            keyboard_state: RefCell::new(KeyboardState::new()),
            mouse_button_counter: Cell::new(0),
            mouse_was_outside_window: true.into(),
            cursor_icon: Cell::new(MouseCursor::Default),
            handler: OnceCell::new(),
            scale_policy,
            user32,

            #[cfg(feature = "opengl")]
            gl_context: OnceCell::new(),
        }
    }

    pub(crate) fn handle_on_frame(&self) {
        let Some(handler) = self.handler.get() else { return };

        handler.on_frame()
    }

    pub(crate) fn handle_event(&self, event: Event) -> EventStatus {
        let Some(handler) = self.handler.get() else {
            return EventStatus::Ignored;
        };

        handler.on_event(event)
    }

    pub fn size(&self) -> WindowSize {
        WindowSize::from_physical(self.current_size.get(), self.scale_factor())
    }

    pub fn scale_factor(&self) -> f64 {
        match self.scale_policy {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => self.current_dpi.get().scale_factor(),
        }
    }

    pub(crate) fn keyboard_state(&self) -> Ref<'_, KeyboardState> {
        self.keyboard_state.borrow()
    }

    pub fn send_resized(&self) {
        if let Some(handler) = self.handler.get() {
            handler
                .resized(WindowSize::from_physical(self.current_size.get(), self.scale_factor()));
        }
    }

    pub fn close(&self) {
        unsafe {
            PostMessageW(self.hwnd, crate::platform::win::window::BV_WINDOW_MUST_CLOSE, 0, 0);
        }
    }

    pub fn has_focus(&self) -> bool {
        HWnd::get_focused_window() == self.hwnd
    }

    pub fn focus(&self) {
        self.hwnd().set_focus().unwrap()
    }

    fn hwnd(&self) -> HWnd {
        // SAFETY: this handle should be safe to use
        unsafe { HWnd::from_raw(self.hwnd) }
    }

    pub fn resize(&self, size: Size) {
        // `self.window_info` will be modified in response to the `WM_SIZE` event that
        // follows the `SetWindowPos()` call
        let dpi = self.current_dpi.get();
        let new_size = size.to_physical(dpi.scale_factor());

        self.hwnd().resize_and_activate(new_size, dpi, &self.user32).unwrap();
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        self.cursor_icon.set(mouse_cursor);
        if let Ok(cursor) = SystemCursor::load(mouse_cursor) {
            cursor.set()
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<crate::gl::GlContext> {
        use std::rc::Rc;
        Some(crate::gl::GlContext::new(Rc::clone(self.gl_context.get()?)))
    }

    pub fn window_handle(&self) -> Option<raw_window_handle::WindowHandle<'_>> {
        let Some(hwnd) = NonZeroIsize::new(self.hwnd as _) else { unreachable!() };
        let handle = Win32WindowHandle::new(hwnd);
        // TODO: add HINSTANCE
        Some(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }

    pub fn display_handle(&self) -> DisplayHandle<'_> {
        DisplayHandle::windows()
    }
}
