use windows_core::{ComObject, Result, HSTRING};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    UI::{
        Controls::WM_MOUSELEAVE,
        WindowsAndMessaging::{
            PostMessageW, HTCLIENT, WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_DPICHANGED,
            WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
            WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SIZE, WM_SYSCHAR,
            WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

use std::cell::{Cell, Ref, RefCell};
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::ptr::null_mut;
use std::rc::Rc;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};

const BV_WINDOW_MUST_CLOSE: u32 = WM_USER + 1;

use super::*;
use crate::{
    Event, EventStatus, MouseButton, MouseCursor, MouseEvent, PhyPoint, PhySize, ScrollDelta, Size,
    WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};

use super::drop_target::DropTarget;
use super::keyboard::KeyboardState;
use crate::wrappers::win32::cursor::SystemCursor;

use crate::wrappers::win32::window::*;
use crate::wrappers::win32::{
    ole_initialize, run_thread_message_loop_until, Dpi, DpiAwarenessContext, ExtendedUser32, Rect,
    WindowStyle,
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
    hwnd: Option<HWND>,
    is_open: Rc<Cell<bool>>,
}

impl WindowHandle {
    pub fn close(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            unsafe {
                PostMessageW(hwnd, BV_WINDOW_MUST_CLOSE, 0, 0);
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.get()
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        if let Some(hwnd) = self.hwnd {
            let mut handle = Win32WindowHandle::empty();
            handle.hwnd = hwnd;

            RawWindowHandle::Win32(handle)
        } else {
            RawWindowHandle::Win32(Win32WindowHandle::empty())
        }
    }
}

struct ParentHandle {
    is_open: Rc<Cell<bool>>,
}

impl Drop for ParentHandle {
    fn drop(&mut self) {
        self.is_open.set(false);
    }
}

type HandlerBuilder = dyn FnOnce(&mut crate::Window) -> Box<dyn WindowHandler>;

pub struct BaseviewWindow {
    window_state: Rc<WindowState>,
    initial_size: Size,

    handler_builder: Cell<Option<Box<HandlerBuilder>>>,

    // Things not directly used, but kept so their Drop impl runs when the window is destroyed
    _parent_handle: ParentHandle,
    _keyboard_hook: Cell<Option<hook::KeyboardHookHandle>>,
    _drop_target: Cell<Option<ComObject<DropTarget>>>,

    #[cfg(feature = "opengl")]
    pub gl_config: Option<crate::gl::GlConfig>,
}

impl WindowImpl for BaseviewWindow {
    fn after_create(&self, window: HWnd) -> Result<()> {
        let hwnd = window.as_raw();
        let window_state = &self.window_state;

        self._keyboard_hook.set(Some(hook::init_keyboard_hook(hwnd)));

        // Now we can get the actual dpi of the window.
        let dpi = window.get_dpi(&self.window_state.user32)?;
        let mut dpi_changed = false;

        if dpi != window_state.current_dpi.get() {
            window_state.current_dpi.set(dpi);
            dpi_changed = true;

            // If the user's requested initial size was in system-scaled logical pixels
            if let WindowScalePolicy::SystemScaleFactor = self.window_state.scale_policy {
                // We cannot create a window in "logical" pixels, and we can't DPI-scale to physical pixels because we
                // have no way to know where the window will end up.
                // So, at window creation, we assume a DPI=96, and if it ends up wrong, we resize the window
                // to the actual logical size the user desired.
                let new_size = WindowInfo::from_logical_size(self.initial_size, dpi.scale_factor())
                    .physical_size();

                // Preemptively update so a synchronous WM_SIZE from SetWindowPos below
                // doesn't also emit Resized.
                window_state.current_size.set(new_size);
                window.resize_and_activate(new_size, dpi, &window_state.user32)?;
            }
        }

        let drop_target = ComObject::new(DropTarget::new(Rc::downgrade(window_state)));
        self._drop_target.set(Some(drop_target.clone()));

        ole_initialize()?;
        window.register_drag_drop(drop_target.as_interface())?;

        #[cfg(feature = "opengl")]
        if let Some(gl_config) = self.gl_config.clone() {
            let mut handle = Win32WindowHandle::empty();
            handle.hwnd = hwnd;
            let handle = RawWindowHandle::Win32(handle);

            let gl_context = unsafe { gl::GlContext::create(&handle, gl_config) }
                .expect("Could not create OpenGL context");

            let Ok(()) = self.window_state.gl_context.set(crate::gl::GlContext::new(gl_context))
            else {
                unreachable!();
            };
        };

        let handler = {
            let mut window = crate::Window::new(Window { state: window_state });

            self.handler_builder.take().unwrap()(&mut window)
        };
        *window_state.handler.borrow_mut() = Some(handler);

        if dpi_changed {
            // Send an initial Resized event so users get the correct scale factor and physical size.
            self.window_state.send_resized();
        }

        Ok(())
    }

    unsafe fn handle_message(
        &self, window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM,
    ) -> Option<LRESULT> {
        let result = unsafe { wnd_proc_inner(window, msg, wparam, lparam, &self.window_state) };

        // If any of the above event handlers caused tasks to be pushed to the deferred tasks list,
        // then we'll try to handle them now
        loop {
            // NOTE: This is written like this instead of using a `while let` loop to avoid exending
            //       the borrow of `window_state.deferred_tasks` into the call of
            //       `window_state.handle_deferred_task()` since that may also generate additional
            //       messages.
            let task = match self.window_state.deferred_tasks.borrow_mut().pop_front() {
                Some(task) => task,
                None => break,
            };

            self.window_state.handle_deferred_task(task, window);
        }

        result
    }

    fn before_destroy(&self, window: HWnd) {
        let _ = window.revoke_drag_drop();
    }
}

/// Our custom `wnd_proc` handler. If the result contains a value, then this is returned after
/// handling any deferred tasks. otherwise the default window procedure is invoked.
unsafe fn wnd_proc_inner(
    window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM, window_state: &WindowState,
) -> Option<LRESULT> {
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

            let physical_pos = PhyPoint { x, y };
            let logical_pos = physical_pos.to_logical(&window_state.window_info());
            let move_event = Event::Mouse(MouseEvent::CursorMoved {
                position: logical_pos,
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

            let new_window_info = {
                let new_size = PhySize { width, height };
                let current_size = window_state.current_size.get();

                // Only send the event if anything changed
                if current_size == new_size {
                    return None;
                }

                window_state.current_size.set(new_size);

                WindowInfo::from_physical_size(new_size, window_state.current_scale_factor())
            };

            window_state.handle_event(Event::Window(WindowEvent::Resized(new_window_info)));

            None
        }
        WM_DPICHANGED => {
            let suggested_nc_rect = Rect((lparam as *const RECT).read());
            let dpi = Dpi((wparam & 0xFFFF) as u16 as u32);

            let dpi_ctx = DpiAwarenessContext::new(&window_state.user32).unwrap();
            let style = window.get_style().unwrap();
            let suggested_rect =
                dpi_ctx.nc_area_to_client_area(suggested_nc_rect, style, dpi).unwrap();

            let new_size = suggested_rect.size();

            let changed = window_state.current_size.get() != new_size
                || window_state.current_dpi.get() != dpi;

            window_state.current_dpi.set(dpi);
            window_state.current_size.set(new_size);

            // Windows makes us resize the window manually. This however will not send a WM_SIZE event,
            // hence why we are notifying the window handler manually below.
            let _ = window.set_nc_rect(suggested_nc_rect);

            if changed {
                window_state.send_resized();
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

/// All data associated with the window. This uses internal mutability so the outer struct doesn't
/// need to be mutably borrowed. Mutably borrowing the entire `WindowState` can be problematic
/// because of the Windows message loops' reentrant nature. Care still needs to be taken to prevent
/// `handler` from indirectly triggering other events that would also need to be handled using
/// `handler`.
pub(crate) struct WindowState {
    /// The HWND belonging to this window. The window's actual state is stored in the `WindowState`
    /// struct associated with this HWND through `unsafe { GetWindowLongPtrW(self.hwnd,
    /// GWLP_USERDATA) } as *const WindowState`.
    pub hwnd: HWND,
    current_size: Cell<PhySize>,
    current_dpi: Cell<Dpi>, // None if in non-system scale policy
    keyboard_state: RefCell<KeyboardState>,
    mouse_button_counter: Cell<usize>,
    mouse_was_outside_window: Cell<bool>,
    cursor_icon: Cell<MouseCursor>,
    // Initialized late so the `Window` can hold a reference to this `WindowState`
    handler: RefCell<Option<Box<dyn WindowHandler>>>,
    scale_policy: WindowScalePolicy,

    user32: ExtendedUser32,

    /// Tasks that should be executed at the end of `wnd_proc`. This is needed to avoid mutably
    /// borrowing the fields from `WindowState` more than once. For instance, when the window
    /// handler requests a resize in response to a keyboard event, the window state will already be
    /// borrowed in `wnd_proc`. So the `resize()` function below cannot also mutably borrow that
    /// window state at the same time.
    pub deferred_tasks: RefCell<VecDeque<WindowTask>>,

    #[cfg(feature = "opengl")]
    pub gl_context: core::cell::OnceCell<crate::gl::GlContext>,
}

impl WindowState {
    pub fn new(
        hwnd: HWND, current_size: PhySize, scale_policy: WindowScalePolicy, user32: ExtendedUser32,
    ) -> Self {
        Self {
            hwnd,
            current_dpi: Dpi::default().into(),
            current_size: current_size.into(),
            keyboard_state: RefCell::new(KeyboardState::new()),
            mouse_button_counter: Cell::new(0),
            mouse_was_outside_window: true.into(),
            cursor_icon: Cell::new(MouseCursor::Default),
            handler: RefCell::new(None),
            scale_policy,
            user32,

            deferred_tasks: RefCell::new(VecDeque::with_capacity(4)),

            #[cfg(feature = "opengl")]
            gl_context: core::cell::OnceCell::new(),
        }
    }

    pub(crate) fn handle_on_frame(&self) {
        let mut handler = self.handler.borrow_mut();
        let Some(handler) = handler.as_mut() else { return };
        let mut window = crate::window::Window::new(Window { state: self });

        handler.on_frame(&mut window)
    }

    pub(crate) fn handle_event(&self, event: Event) -> EventStatus {
        let mut handler = self.handler.borrow_mut();

        let Some(handler) = handler.as_mut() else {
            return EventStatus::Ignored;
        };

        let mut window = crate::window::Window::new(Window { state: self });
        handler.on_event(&mut window, event)
    }

    pub(crate) fn window_info(&self) -> WindowInfo {
        WindowInfo::from_physical_size(self.current_size.get(), self.current_scale_factor())
    }

    fn current_scale_factor(&self) -> f64 {
        match self.scale_policy {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => self.current_dpi.get().scale_factor(),
        }
    }

    pub(crate) fn keyboard_state(&self) -> Ref<'_, KeyboardState> {
        self.keyboard_state.borrow()
    }

    fn send_resized(&self) {
        self.handle_event(Event::Window(WindowEvent::Resized(self.window_info())));
    }

    /// Handle a deferred task as described in [`Self::deferred_tasks`].
    pub(self) fn handle_deferred_task(&self, task: WindowTask, window: HWnd) {
        match task {
            WindowTask::Resize(size) => {
                // `self.window_info` will be modified in response to the `WM_SIZE` event that
                // follows the `SetWindowPos()` call
                let dpi = self.current_dpi.get();
                let window_info = WindowInfo::from_logical_size(size, dpi.scale_factor());
                let new_size = window_info.physical_size();

                window.resize_and_activate(new_size, dpi, &self.user32).unwrap();
            }
            WindowTask::Focus => window.set_focus().unwrap(),
        }
    }
}

/// Tasks that must be deferred until the end of [`wnd_proc()`] to avoid reentrant `WindowState`
/// borrows. See the docstring on [`WindowState::deferred_tasks`] for more information.
#[derive(Debug, Clone)]
pub(crate) enum WindowTask {
    /// Resize the window to the given size. The size is in logical pixels. DPI scaling is applied
    /// automatically.
    Resize(Size),
    /// Request keyboard focus for the window.
    Focus,
}

pub struct Window<'a> {
    state: &'a WindowState,
}

impl Window<'_> {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let parent = match parent.raw_window_handle() {
            RawWindowHandle::Win32(h) => h.hwnd,
            h => panic!("unsupported parent handle {:?}", h),
        };

        Self::open(true, parent, options, build)
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let window_handle = Self::open(false, null_mut(), options, build);

        run_thread_message_loop_until(|| !window_handle.is_open()).unwrap();
    }

    fn open<H, B>(
        parented: bool, parent: HWND, options: WindowOpenOptions, build: B,
    ) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let extended_user_32 = ExtendedUser32::load().unwrap();
        let title = HSTRING::from(options.title);

        let scaling_factor = match options.scale {
            WindowScalePolicy::SystemScaleFactor => 1.0,
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };

        let window_size =
            WindowInfo::from_logical_size(options.size, scaling_factor).physical_size();

        let style = if parented { WindowStyle::parented() } else { WindowStyle::embedded() };
        let dpi_ctx = DpiAwarenessContext::new(&extended_user_32).unwrap();

        let rect =
            dpi_ctx.client_area_to_nc_area(window_size.into(), style, Dpi::default()).unwrap();

        let is_open = Rc::new(Cell::new(true));

        let parent_handle = ParentHandle { is_open: is_open.clone() };

        let initializer = move |hwnd: HWnd| {
            let window_state = Rc::new(WindowState::new(
                hwnd.as_raw(),
                window_size,
                options.scale,
                extended_user_32,
            ));

            BaseviewWindow {
                window_state,
                initial_size: options.size,
                handler_builder: Cell::new(Some(Box::new(|w| Box::new(build(w))))),

                _parent_handle: parent_handle,
                _drop_target: None.into(),
                _keyboard_hook: None.into(),

                #[cfg(feature = "opengl")]
                gl_config: options.gl_config,
            }
        };

        let hwnd =
            create_window(&title, style, rect.size(), parent as *mut _, &dpi_ctx, initializer)
                .unwrap();

        // SAFETY: this handle should be safe to use
        let window = unsafe { HWnd::from_raw(hwnd) };

        // FIXME: this SetTimer call could be in after_create, but for some reason it changes the ordering
        // for a parent+child window situation, which results in the parent drawing over the child.
        // This timer should be replaced by proper window redrawing/damage/vsync handling, but this
        // would be a breaking change, so we'll do that later.
        // TODO: create a new timer instead of hard-coding a specific ID
        window.set_timer(WIN_FRAME_TIMER, 15).unwrap();

        window.show_and_activate();

        WindowHandle { hwnd: Some(hwnd), is_open: Rc::clone(&is_open) }
    }

    pub fn close(&mut self) {
        unsafe {
            PostMessageW(self.state.hwnd, BV_WINDOW_MUST_CLOSE, 0, 0);
        }
    }

    pub fn has_focus(&mut self) -> bool {
        HWnd::get_focused_window() == self.state.hwnd
    }

    pub fn focus(&mut self) {
        // To avoid reentrant event handler calls we'll defer the actual focus request until after
        // the event has been handled
        self.state.deferred_tasks.borrow_mut().push_back(WindowTask::Focus);
    }

    pub fn resize(&mut self, size: Size) {
        // To avoid reentrant event handler calls we'll defer the actual resizing until after the
        // event has been handled
        let task = WindowTask::Resize(size);
        self.state.deferred_tasks.borrow_mut().push_back(task);
    }

    pub fn set_mouse_cursor(&mut self, mouse_cursor: MouseCursor) {
        self.state.cursor_icon.set(mouse_cursor);
        if let Ok(cursor) = SystemCursor::load(mouse_cursor) {
            cursor.set()
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&crate::gl::GlContext> {
        self.state.gl_context.get()
    }
}

unsafe impl HasRawWindowHandle for Window<'_> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = self.state.hwnd;

        RawWindowHandle::Win32(handle)
    }
}

unsafe impl HasRawDisplayHandle for Window<'_> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
