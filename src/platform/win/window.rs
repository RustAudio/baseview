use windows_core::{ComObject, HSTRING};
use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, RECT, WPARAM},
    UI::{
        Controls::WM_MOUSELEAVE,
        WindowsAndMessaging::{
            HTCLIENT, WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_DPICHANGED, WM_INPUTLANGCHANGE,
            WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
            WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN,
            WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN,
            WM_SYSKEYUP, WM_TIMER, WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

use crate::{warn, HandlerError};
use dpi::{PhysicalPosition, PhysicalSize, Size};
use std::cell::Cell;
use std::num::NonZeroUsize;

pub(crate) const BV_WINDOW_MUST_CLOSE: u32 = WM_USER + 1;

use super::drop_target::DropTarget;
use super::*;
use crate::handler::WindowHandlerBuilder;
use crate::host::Host;
use crate::platform::win::window_state::{WindowSharedState, WindowState};
use crate::platform::Error;
use crate::wrappers::win32::cursor::SystemCursor;
use crate::wrappers::win32::window::*;
use crate::wrappers::win32::{
    ole_initialize, run_thread_message_loop_until, Dpi, DpiAwarenessContext, ExtendedUser32, Rect,
    WindowStyle,
};
use crate::{
    Event, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowOpenOptions, WindowSize,
};

#[allow(non_snake_case)]
fn HIWORD(wparam: WPARAM) -> u16 {
    ((wparam >> 16) & 0xffff) as u16
}

#[allow(non_snake_case)]
fn LOWORD(lparam: LPARAM) -> u16 {
    (lparam & 0xffff) as u16
}

const WIN_FRAME_TIMER: NonZeroUsize = match NonZeroUsize::new(4242) {
    Some(x) => x,
    None => unreachable!(),
};

pub struct WindowHandle {
    hwnd: Cell<Option<HWnd>>,
    state: Rc<WindowSharedState>,
}

impl WindowHandle {
    pub fn run_until_closed(self) -> Result<()> {
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
        let Some(hwnd) = self.hwnd.get() else { return Ok(()) };
        let new_size = new_size.to_physical(self.state.scale_factor());
        let _guard = self.state.originate_host_resize();
        hwnd.resize_and_activate(new_size, self.state.current_dpi.get(), &self.state.user32)?;

        if self.state.current_size.get() == new_size {
            Ok(())
        } else {
            Err(Error::ResizeFailed)
        }
    }

    pub fn suggest_scale_factor(&self, scale_factor: f64) -> Result<()> {
        let Some(hwnd) = self.hwnd.get() else { return Ok(()) };

        let current_scale_factor = self.state.scale_factor();
        self.state.fallback_scale_factor.set(Some(scale_factor));

        if self.state.current_dpi.get().is_some() {
            return Ok(());
        }

        let current_size = self.state.current_size.get();
        let new_size = self
            .state
            .current_size
            .get()
            .to_logical::<f64>(current_scale_factor)
            .to_physical(self.state.scale_factor());

        // This call doesn't meaningfully change the scaling factor, ignore the result
        if current_size == new_size {
            return Ok(());
        }

        let _guard = self.state.originate_host_resize();

        hwnd.resize_and_activate(new_size, None, &self.state.user32)?;

        if self.state.current_size.get() == new_size {
            Ok(())
        } else {
            Err(Error::ResizeFailed)
        }
    }

    pub fn set_parent(&self, new_parent: ParentWindowHandle) -> Result<()> {
        let Some(hwnd) = self.hwnd.get() else { return Ok(()) };

        hwnd.set_parent(&new_parent.handle)?;

        if !self.state.parented.get() {
            self.state.parented.set(true);

            hwnd.set_style(WindowStyle::parented())?;
        }

        Ok(())
    }

    #[inline]
    pub fn handle_main_thread_callback(&self) {
        // No-op
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
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
    host: Host,

    // Things not directly used, but kept so their Drop impl runs when the window is destroyed
    _keyboard_hook: Cell<Option<hook::KeyboardHookHandle>>,
    _drop_target: Cell<Option<ComObject<DropTarget>>>,

    #[cfg(feature = "opengl")]
    pub gl_config: Option<crate::gl::GlConfig>,
}

impl BaseviewWindow {
    fn notify_destroyed_to_host(&self) {
        if self.shared_state.destroy_host_originated.get() {
            return;
        };

        self.host.notify_destroyed()
    }

    fn request_resize_from_host(
        &self, new_size: WindowSize,
    ) -> core::result::Result<(), HandlerError> {
        if self.shared_state.resize_host_originated.get() {
            return Ok(());
        };

        self.host.request_resize(new_size)
    }
}

impl Drop for BaseviewWindow {
    fn drop(&mut self) {
        self.shared_state.is_alive.set(false);
        self.notify_destroyed_to_host();
    }
}

impl WindowImpl for BaseviewWindow {
    fn after_create(&self, window: HWnd) -> core::result::Result<(), Error> {
        let hwnd = window.as_raw();
        let window_state = &self.window_state;

        self._keyboard_hook.set(Some(hook::init_keyboard_hook(hwnd)));

        // Now we can get the actual dpi of the window.
        let dpi = window.get_dpi(&self.window_state.user32)?;

        if let Some(dpi) = dpi {
            if Some(dpi) != window_state.shared.current_dpi.get() {
                window_state.shared.current_dpi.set(Some(dpi));

                // We cannot create a window in "logical" pixels, and we can't DPI-scale to physical pixels because we
                // have no way to know where the window will end up.
                // So, at window creation, we assume a DPI=96, and if it ends up wrong, we resize the window
                // to the actual logical size the user desired.
                let new_size = self.initial_size.to_physical(dpi.scale_factor());

                // Preemptively update so a synchronous WM_SIZE from SetWindowPos below
                // doesn't also emit Resized.
                window_state.shared.current_size.set(new_size);
                window.resize_and_activate(new_size, Some(dpi), &window_state.user32)?;
            }
        }

        let drop_target = ComObject::new(DropTarget::new(Rc::downgrade(window_state), window));
        self._drop_target.set(Some(drop_target.clone()));

        ole_initialize()?;
        window.register_drag_drop(drop_target.as_interface())?;

        #[cfg(feature = "opengl")]
        if let Some(gl_config) = self.gl_config.clone() {
            let gl_context = gl::GlContextInner::create(window, gl_config)
                .expect("Could not create OpenGL context");

            let Ok(()) = self.window_state.gl_context.set(Rc::new(gl_context)) else {
                unreachable!();
            };
        };

        let handler = {
            let context = crate::WindowContext::new(Rc::clone(&self.window_state));
            self.handler_builder.take().unwrap().build(context)?
        };
        let Ok(()) = window_state.handler.set(handler) else { unreachable!() };

        Ok(())
    }

    unsafe fn handle_message(
        &self, window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM,
    ) -> Option<LRESULT> {
        unsafe { wnd_proc_inner(window, msg, wparam, lparam, self) }
    }

    fn before_destroy(&self, window: HWnd) {
        let _ = window.revoke_drag_drop();
    }
}

/// Our custom `wnd_proc` handler. If the result contains a value, then this is returned after
/// handling any deferred tasks. otherwise the default window procedure is invoked.
unsafe fn wnd_proc_inner(
    window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM, window_bv: &BaseviewWindow,
) -> Option<LRESULT> {
    let window_state = &window_bv.window_state;
    match msg {
        WM_MOUSEMOVE => {
            if window_state.mouse_was_outside_window.get() {
                // this makes Windows track whether the mouse leaves the window.
                // When the mouse leaves it results in a `WM_MOUSELEAVE` event.
                // Couldn't find a good way to track whether the mouse enters,
                // but if `WM_MOUSEMOVE` happens, the mouse must have entered.
                let _ = window.start_cursor_leave_tracking();
                window_state.mouse_was_outside_window.set(false);

                let enter_event = Event::Mouse(MouseEvent::CursorEntered);
                window_state.handle_event(enter_event);
            }

            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let move_event = Event::Mouse(MouseEvent::CursorMoved {
                position: PhysicalPosition { x, y }.cast(),
                modifiers: window_state
                    .keyboard_state
                    .borrow()
                    .get_modifiers_from_mouse_wparam(wparam),
            });

            window_state.handle_event(move_event);
            Some(0)
        }

        WM_MOUSELEAVE => {
            window_state.handle_event(Event::Mouse(MouseEvent::CursorLeft));

            window_state.mouse_was_outside_window.set(true);
            Some(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / WHEEL_DELTA as f32;

            let event = Event::Mouse(MouseEvent::WheelScrolled {
                delta: if msg == WM_MOUSEWHEEL {
                    ScrollDelta::Lines { x: 0.0, y: value }
                } else {
                    ScrollDelta::Lines { x: value, y: 0.0 }
                },
                modifiers: window_state
                    .keyboard_state
                    .borrow()
                    .get_modifiers_from_mouse_wparam(wparam),
            });

            window_state.handle_event(event);
            Some(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_RBUTTONDOWN
        | WM_RBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let mut mouse_button_counter = window_state.mouse_button_counter.get();

            #[allow(non_snake_case)]
            fn GET_XBUTTON_WPARAM(wparam: WPARAM) -> u16 {
                HIWORD(wparam)
            }

            const XBUTTON1: u16 = 0x1;
            const XBUTTON2: u16 = 0x2;

            let button = match msg {
                WM_LBUTTONDOWN | WM_LBUTTONUP => Some(MouseButton::Left),
                WM_MBUTTONDOWN | WM_MBUTTONUP => Some(MouseButton::Middle),
                WM_RBUTTONDOWN | WM_RBUTTONUP => Some(MouseButton::Right),
                WM_XBUTTONDOWN | WM_XBUTTONUP => match GET_XBUTTON_WPARAM(wparam) {
                    XBUTTON1 => Some(MouseButton::Back),
                    XBUTTON2 => Some(MouseButton::Forward),
                    _ => None,
                },
                _ => None,
            };

            if let Some(button) = button {
                let event = match msg {
                    WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN | WM_XBUTTONDOWN => {
                        // Capture the mouse cursor on button down
                        mouse_button_counter = mouse_button_counter.saturating_add(1);
                        window.set_capture();
                        MouseEvent::ButtonPressed {
                            button,
                            modifiers: window_state
                                .keyboard_state
                                .borrow()
                                .get_modifiers_from_mouse_wparam(wparam),
                        }
                    }
                    WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP | WM_XBUTTONUP => {
                        // Release the mouse cursor capture when all buttons are released
                        mouse_button_counter = mouse_button_counter.saturating_sub(1);
                        if mouse_button_counter == 0 {
                            HWnd::release_capture();
                        }

                        MouseEvent::ButtonReleased {
                            button,
                            modifiers: window_state
                                .keyboard_state
                                .borrow()
                                .get_modifiers_from_mouse_wparam(wparam),
                        }
                    }
                    _ => {
                        unreachable!()
                    }
                };

                window_state.mouse_button_counter.set(mouse_button_counter);
                window_state.handle_event(Event::Mouse(event));
            }

            None
        }
        WM_TIMER => {
            if wparam == WIN_FRAME_TIMER.get() {
                window_state.handle_on_frame()
            }

            Some(0)
        }
        WM_CLOSE => {
            window_state.handle_event(Event::Window(WindowEvent::WillClose));

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
                window_state.handle_event(Event::Keyboard(event));
            }

            if msg != WM_SYSKEYDOWN {
                Some(0)
            } else {
                None
            }
        }
        WM_SETFOCUS => {
            window_state.handle_event(Event::Window(WindowEvent::Focused));

            None
        }
        WM_KILLFOCUS => {
            window_state.handle_event(Event::Window(WindowEvent::Unfocused));

            None
        }
        WM_SIZE => {
            let width = (lparam & 0xFFFF) as u16 as u32;
            let height = ((lparam >> 16) & 0xFFFF) as u16 as u32;

            let new_size = PhysicalSize { width, height };
            let current_size = window_state.shared.current_size.get();

            // Only send the event if anything changed
            if current_size == new_size {
                return None;
            }

            let previous = window_state.shared.current_size.replace(new_size);
            let new_size = WindowSize::from_physical(new_size, window_state.shared.scale_factor());

            let handler = window_state.handler.get()?;
            if let Err(e) = handler.resized(new_size) {
                warn!("Window Handler failed to resize: {}", e);
                window_state.shared.current_size.set(previous);

                if let Err(e) = window_state.resize(previous.into()) {
                    warn!("Failed to resize back to previous window size: {}", e);
                }

                return Some(-1);
            }

            if let Err(e) = window_bv.request_resize_from_host(new_size) {
                warn!("Resize request from Host failed: {}. Reverting to previous size.", e);

                if let Err(e) = handler.resized(new_size) {
                    warn!("Window Handler failed to resize to previous window size: {}", e);
                }

                window_state.shared.current_size.set(previous);
                if let Err(e) = window_state.resize(previous.into()) {
                    warn!("Failed to resize back to previous window size: {}", e);
                }

                return Some(-1);
            }

            None
        }
        WM_DPICHANGED => {
            let suggested_nc_rect = Rect((lparam as *const RECT).read());
            let dpi = Dpi((wparam & 0xFFFF) as u16 as u32);

            let dpi_ctx = DpiAwarenessContext::new(&window_state.user32).unwrap();
            let style = window.get_style().unwrap();
            let suggested_rect =
                dpi_ctx.nc_area_to_client_area(suggested_nc_rect, style, Some(dpi)).unwrap();

            let new_size = suggested_rect.size();

            let changed = window_state.shared.current_size.get() != new_size
                || window_state.shared.current_dpi.get() != Some(dpi);

            window_state.shared.current_dpi.replace(Some(dpi));
            let previous_size = window_state.shared.current_size.replace(new_size);

            // Windows makes us resize the window manually. This however will not send a WM_SIZE event,
            // hence why we are notifying the window handler manually below.
            let _ = window.set_nc_rect(suggested_nc_rect);

            if changed {
                let handler = window_state.handler.get()?;
                let new_size = WindowSize::from_physical(new_size, dpi.scale_factor());

                if let Err(e) = handler.resized(new_size) {
                    warn!("Window Handler failed to resize: {}", e);
                    window_state.shared.current_size.set(previous_size);

                    if let Err(e) = window_state.resize(previous_size.into()) {
                        warn!("Failed to resize back to previous window size: {}", e);
                    }
                }

                if let Err(e) = window_bv.request_resize_from_host(new_size) {
                    warn!("Resize request from Host failed: {}. Reverting to previous size.", e);

                    if let Err(e) = handler.resized(new_size) {
                        warn!("Window Handler failed to resize to previous window size: {}", e);
                    }

                    window_state.shared.current_size.set(previous_size);
                    if let Err(e) = window_state.resize(previous_size.into()) {
                        warn!("Failed to resize back to previous window size: {}", e);
                    }

                    return Some(-1);
                }
            }

            None
        }
        // If WM_SETCURSOR returns `None`, WM_SETCURSOR continues to get handled by the outer window(s),
        // If it returns `Some(1)`, the current window decides what the cursor is
        WM_SETCURSOR => {
            let low_word = LOWORD(lparam) as u32;
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
        _ => None,
    }
}

impl WindowHandle {
    pub fn create_window(
        options: WindowOpenOptions, build: WindowHandlerBuilder, host: Host,
    ) -> Result<WindowHandle> {
        let extended_user_32 = ExtendedUser32::load()?;
        let title = HSTRING::from(options.title);

        let scaling_factor = 1.0;

        let window_size = options.size.to_physical(scaling_factor);

        let style = if options.parent.is_some() {
            WindowStyle::parented()
        } else {
            WindowStyle::embedded()
        };
        let dpi_ctx = DpiAwarenessContext::new(&extended_user_32)?;
        let shared_state =
            WindowSharedState::new(window_size, extended_user_32.clone(), options.parent.is_some());

        let initializer = {
            let extended_user_32 = extended_user_32.clone();
            let shared_state = shared_state.clone();

            move |hwnd: HWnd| {
                let window_state =
                    Rc::new(WindowState::new(hwnd, extended_user_32, shared_state.clone()));

                BaseviewWindow {
                    window_state,
                    initial_size: options.size,
                    handler_builder: Cell::new(Some(build)),
                    shared_state,
                    host,

                    _drop_target: None.into(),
                    _keyboard_hook: None.into(),

                    #[cfg(feature = "opengl")]
                    gl_config: options.gl_config,
                }
            }
        };

        let parent = options.parent.map(|p| p.inner.handle);
        let rect = dpi_ctx.client_area_to_nc_area(window_size.into(), style, None)?;

        let window = create_window(&title, style, rect.size(), parent, &dpi_ctx, initializer)?;

        // FIXME: this SetTimer call could be in after_create, but for some reason it changes the ordering
        // for a parent+child window situation, which results in the parent drawing over the child.
        // This timer should be replaced by proper window redrawing/damage/vsync handling, but this
        // would be a breaking change, so we'll do that later.
        // TODO: create a new timer instead of hard-coding a specific ID
        window.set_timer(WIN_FRAME_TIMER, 15)?;

        window.show_and_activate();

        Ok(WindowHandle { hwnd: Some(window).into(), state: shared_state })
    }
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
