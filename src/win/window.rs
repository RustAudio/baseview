use windows_core::{ComObject, Interface, Result, HSTRING};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    System::Ole::{OleInitialize, RevokeDragDrop},
    UI::{
        Controls::{HOVER_DEFAULT, WM_MOUSELEAVE},
        HiDpi::{
            GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
        },
        Input::KeyboardAndMouse::{
            GetFocus, ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE,
            TRACKMOUSEEVENT,
        },
        WindowsAndMessaging::{
            AdjustWindowRectEx, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            LoadCursorW, PostMessageW, SetCursor, SetTimer, SetWindowPos, TranslateMessage,
            HTCLIENT, MSG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, WHEEL_DELTA, WM_CHAR,
            WM_CLOSE, WM_DPICHANGED, WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
            WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS,
            WM_SHOWWINDOW, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_USER,
            WM_XBUTTONDOWN, WM_XBUTTONUP, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX,
            WS_MINIMIZEBOX, WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE,
        },
    },
};

use std::cell::{Cell, Ref, RefCell};
use std::collections::VecDeque;
use std::ptr::null_mut;
use std::rc::Rc;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use windows::Win32::System::Ole::IDropTarget;
use windows_sys::Win32::Foundation::FALSE;
use windows_sys::Win32::System::Ole::RegisterDragDrop;

const BV_WINDOW_MUST_CLOSE: u32 = WM_USER + 1;

use crate::win::hook::{self, KeyboardHookHandle};
use crate::{
    Event, EventStatus, MouseButton, MouseCursor, MouseEvent, PhyPoint, PhySize, ScrollDelta, Size,
    WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};

use super::cursor::cursor_to_lpcwstr;
use super::drop_target::DropTarget;
use super::keyboard::KeyboardState;

#[cfg(feature = "opengl")]
use crate::gl::GlContext;
use crate::wrappers::win32::window::*;

#[allow(non_snake_case)]
fn HIWORD(wparam: WPARAM) -> u16 {
    ((wparam >> 16) & 0xffff) as u16
}

#[allow(non_snake_case)]
fn LOWORD(lparam: LPARAM) -> u16 {
    (lparam & 0xffff) as u16
}

const WIN_FRAME_TIMER: usize = 4242;

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
    _keyboard_hook: Cell<Option<KeyboardHookHandle>>,
    _drop_target: Cell<Option<ComObject<DropTarget>>>,

    #[cfg(feature = "opengl")]
    pub gl_config: Option<crate::gl::GlConfig>,
}

impl WindowImpl for BaseviewWindow {
    fn after_create(&self, window: HWnd) -> Result<()> {
        let hwnd = window.as_raw();
        let window_state = &self.window_state;

        self._keyboard_hook.set(Some(hook::init_keyboard_hook(hwnd)));

        unsafe {
            // Only works on Windows 10 unfortunately.
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);

            // Now we can get the actual dpi of the window.
            let new_rect = if let WindowScalePolicy::SystemScaleFactor =
                self.window_state.scale_policy
            {
                // Only works on Windows 10 unfortunately.
                let dpi = GetDpiForWindow(hwnd);
                let scale_factor = dpi as f64 / 96.0;

                let current_scale_factor = window_state.current_scale_factor.get();

                if current_scale_factor != scale_factor {
                    window_state.current_scale_factor.set(scale_factor);

                    let new_size = WindowInfo::from_logical_size(self.initial_size, scale_factor)
                        .physical_size();
                    // Preemptively update so a synchronous WM_SIZE from SetWindowPos below
                    // doesn't also emit Resized.
                    window_state.current_size.set(new_size);

                    Some(RECT {
                        left: 0,
                        top: 0,
                        // todo: check if usize fits into i32
                        right: new_size.width as i32,
                        bottom: new_size.height as i32,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let drop_target = ComObject::new(DropTarget::new(Rc::downgrade(window_state)));
            self._drop_target.set(Some(drop_target.clone()));

            OleInitialize(null_mut());

            RegisterDragDrop(hwnd, drop_target.as_interface::<IDropTarget>().as_raw());

            if let Some(mut new_rect) = new_rect {
                // Convert this desired"client rectangle" size to the actual "window rectangle"
                // size (Because of course you have to do that).
                AdjustWindowRectEx(&mut new_rect, window_state.dw_style, 0, 0);

                // Windows makes us resize the window manually. This will trigger another `WM_SIZE` event, but it happens before GWLP_USERDATA is set, so it is not delivered to the handler
                SetWindowPos(
                    hwnd,
                    hwnd,
                    new_rect.left,
                    new_rect.top,
                    new_rect.right - new_rect.left,
                    new_rect.bottom - new_rect.top,
                    SWP_NOZORDER | SWP_NOMOVE,
                );

                // Send an initial Resized event so users get the correct scale factor and physical size.
                self.window_state.send_resized(self.initial_size);
            }
        }

        #[cfg(feature = "opengl")]
        if let Some(gl_config) = self.gl_config.clone() {
            let mut handle = Win32WindowHandle::empty();
            handle.hwnd = hwnd;
            let handle = RawWindowHandle::Win32(handle);

            let gl_context = unsafe { GlContext::create(&handle, gl_config) }
                .expect("Could not create OpenGL context");

            let Ok(()) = self.window_state.gl_context.set(gl_context) else {
                unreachable!();
            };
        };

        let handler = {
            let mut window = crate::Window::new(Window { state: window_state });

            self.handler_builder.take().unwrap()(&mut window)
        };
        *window_state.handler.borrow_mut() = Some(handler);

        Ok(())
    }

    unsafe fn handle_message(
        &self, window: HWnd, msg: u32, wparam: WPARAM, lparam: LPARAM,
    ) -> Option<LRESULT> {
        let hwnd = window.as_raw();

        let result = unsafe { wnd_proc_inner(hwnd, msg, wparam, lparam, &self.window_state) };

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

            self.window_state.handle_deferred_task(task);
        }

        result
    }

    fn before_destroy(&self, window: HWnd) {
        unsafe { RevokeDragDrop(window.as_raw()) };
    }
}

/// Our custom `wnd_proc` handler. If the result contains a value, then this is returned after
/// handling any deferred tasks. otherwise the default window procedure is invoked.
unsafe fn wnd_proc_inner(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, window_state: &WindowState,
) -> Option<LRESULT> {
    match msg {
        WM_MOUSEMOVE => {
            if window_state.mouse_was_outside_window.get() {
                // this makes Windows track whether the mouse leaves the window.
                // When the mouse leaves it results in a `WM_MOUSELEAVE` event.
                let mut track_mouse = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: HOVER_DEFAULT,
                };
                // Couldn't find a good way to track whether the mouse enters,
                // but if `WM_MOUSEMOVE` happens, the mouse must have entered.
                TrackMouseEvent(&mut track_mouse);
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
                        SetCapture(hwnd);
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
                            ReleaseCapture();
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
            if wparam == WIN_FRAME_TIMER {
                window_state.handle_on_frame()
            }

            Some(0)
        }
        WM_CLOSE => {
            window_state.handle_event(Event::Window(WindowEvent::WillClose));

            // DestroyWindow(hwnd);
            // Some(0)
            Some(DefWindowProcW(hwnd, msg, wparam, lparam))
        }
        WM_CHAR | WM_SYSCHAR | WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
        | WM_INPUTLANGCHANGE => {
            let opt_event =
                window_state.keyboard_state.borrow_mut().process_message(hwnd, msg, wparam, lparam);

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

                WindowInfo::from_physical_size(new_size, window_state.current_scale_factor.get())
            };

            window_state.handle_event(Event::Window(WindowEvent::Resized(new_window_info)));

            None
        }
        WM_DPICHANGED => {
            let new_rect = (lparam as *const RECT).read();

            let current_size = window_state.current_size.get();
            let new_size = PhySize {
                width: (new_rect.right - new_rect.left) as u32,
                height: (new_rect.bottom - new_rect.top) as u32,
            };

            let mut changed = current_size != new_size;

            if let WindowScalePolicy::SystemScaleFactor = window_state.scale_policy {
                let dpi = (wparam & 0xFFFF) as u16 as u32;
                let scale_factor = dpi as f64 / 96.0;

                changed |= window_state.current_scale_factor.get() != scale_factor;

                window_state.current_scale_factor.set(scale_factor);
            }

            // Windows makes us resize the window manually. This however will not send a WM_SIZE event,
            // hence why we are notifying the window handler manually below.
            SetWindowPos(
                hwnd,
                null_mut(),
                new_rect.left,
                new_rect.top,
                new_rect.right - new_rect.left,
                new_rect.bottom - new_rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            if changed {
                window_state.current_size.set(new_size);

                let new_window_info = WindowInfo::from_physical_size(
                    new_size,
                    window_state.current_scale_factor.get(),
                );
                window_state.handle_event(Event::Window(WindowEvent::Resized(new_window_info)));
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
                let cursor =
                    LoadCursorW(null_mut(), cursor_to_lpcwstr(window_state.cursor_icon.get()));
                unsafe {
                    SetCursor(cursor);
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
            DestroyWindow(hwnd);
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
pub(super) struct WindowState {
    /// The HWND belonging to this window. The window's actual state is stored in the `WindowState`
    /// struct associated with this HWND through `unsafe { GetWindowLongPtrW(self.hwnd,
    /// GWLP_USERDATA) } as *const WindowState`.
    pub hwnd: HWND,
    current_size: Cell<PhySize>,
    current_scale_factor: Cell<f64>,
    keyboard_state: RefCell<KeyboardState>,
    mouse_button_counter: Cell<usize>,
    mouse_was_outside_window: Cell<bool>,
    cursor_icon: Cell<MouseCursor>,
    // Initialized late so the `Window` can hold a reference to this `WindowState`
    handler: RefCell<Option<Box<dyn WindowHandler>>>,
    scale_policy: WindowScalePolicy,
    dw_style: u32,

    /// Tasks that should be executed at the end of `wnd_proc`. This is needed to avoid mutably
    /// borrowing the fields from `WindowState` more than once. For instance, when the window
    /// handler requests a resize in response to a keyboard event, the window state will already be
    /// borrowed in `wnd_proc`. So the `resize()` function below cannot also mutably borrow that
    /// window state at the same time.
    pub deferred_tasks: RefCell<VecDeque<WindowTask>>,

    #[cfg(feature = "opengl")]
    pub gl_context: core::cell::OnceCell<GlContext>,
}

impl WindowState {
    pub fn new(
        hwnd: HWND, current_size: PhySize, scaling: f64, scale_policy: WindowScalePolicy,
        style_flags: u32,
    ) -> Self {
        Self {
            hwnd,
            current_scale_factor: scaling.into(),
            current_size: current_size.into(),
            keyboard_state: RefCell::new(KeyboardState::new()),
            mouse_button_counter: Cell::new(0),
            mouse_was_outside_window: true.into(),
            cursor_icon: Cell::new(MouseCursor::Default),
            handler: RefCell::new(None),
            scale_policy,
            dw_style: style_flags,

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

    pub(super) fn window_info(&self) -> WindowInfo {
        WindowInfo::from_physical_size(self.current_size.get(), self.current_scale_factor.get())
    }

    pub(super) fn keyboard_state(&self) -> Ref<'_, KeyboardState> {
        self.keyboard_state.borrow()
    }

    fn send_resized(&self, logical_size: Size) {
        let window_info =
            WindowInfo::from_logical_size(logical_size, self.current_scale_factor.get());
        self.current_size.set(window_info.physical_size());

        self.handle_event(Event::Window(WindowEvent::Resized(window_info)));
    }

    /// Handle a deferred task as described in [`Self::deferred_tasks`].
    pub(self) fn handle_deferred_task(&self, task: WindowTask) {
        match task {
            WindowTask::Resize(size) => {
                // `self.window_info` will be modified in response to the `WM_SIZE` event that
                // follows the `SetWindowPos()` call
                let scaling = self.current_scale_factor.get();
                let window_info = WindowInfo::from_logical_size(size, scaling);

                // If the window is a standalone window then the size needs to include the window
                // decorations
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: window_info.physical_size().width as i32,
                    bottom: window_info.physical_size().height as i32,
                };
                unsafe {
                    AdjustWindowRectEx(&mut rect, self.dw_style, 0, 0);
                    SetWindowPos(
                        self.hwnd,
                        self.hwnd,
                        0,
                        0,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOMOVE,
                    )
                };
            }
            WindowTask::Focus => unsafe {
                SetFocus(self.hwnd);
            },
        }
    }
}

/// Tasks that must be deferred until the end of [`wnd_proc()`] to avoid reentrant `WindowState`
/// borrows. See the docstring on [`WindowState::deferred_tasks`] for more information.
#[derive(Debug, Clone)]
pub(super) enum WindowTask {
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

        unsafe {
            let mut msg: MSG = std::mem::zeroed();

            loop {
                let status = GetMessageW(&mut msg, null_mut(), 0, 0);

                if status == -1 {
                    break;
                }

                TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if !window_handle.is_open() {
                    break;
                }
            }
        }
    }

    fn open<H, B>(
        parented: bool, parent: HWND, options: WindowOpenOptions, build: B,
    ) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let title = HSTRING::from(options.title);

        let scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => 1.0,
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };

        let current_size = WindowInfo::from_logical_size(options.size, scaling).physical_size();

        let mut rect = RECT {
            left: 0,
            top: 0,
            // todo: check if usize fits into i32
            right: current_size.width as i32,
            bottom: current_size.height as i32,
        };

        let flags = if parented {
            WS_CHILD | WS_VISIBLE
        } else {
            WS_POPUPWINDOW
                | WS_CAPTION
                | WS_VISIBLE
                | WS_SIZEBOX
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_CLIPSIBLINGS
        };

        if !parented {
            unsafe { AdjustWindowRectEx(&mut rect, flags, FALSE, 0) };
        }

        let is_open = Rc::new(Cell::new(true));

        let parent_handle = ParentHandle { is_open: is_open.clone() };

        let initializer = move |hwnd: HWnd| {
            let window_state = Rc::new(WindowState::new(
                hwnd.as_raw(),
                current_size,
                scaling,
                options.scale,
                flags,
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

        let hwnd = create_window(
            &title,
            flags,
            rect.right - rect.left,
            rect.bottom - rect.top,
            parent as *mut _,
            initializer,
        )
        .unwrap();

        // FIXME: this SetTimer call could be in after_create, but for some reason it changes the ordering
        // for a parent+child window situation, which results in the parent drawing over the child.
        // This timer should be replaced by proper window redrawing/damage/vsync handling, but this
        // would be a breaking change, so we'll do that later.
        unsafe { SetTimer(hwnd, WIN_FRAME_TIMER, 15, None) };

        unsafe { PostMessageW(hwnd, WM_SHOWWINDOW, 0, 0) };

        WindowHandle { hwnd: Some(hwnd), is_open: Rc::clone(&is_open) }
    }

    pub fn close(&mut self) {
        unsafe {
            PostMessageW(self.state.hwnd, BV_WINDOW_MUST_CLOSE, 0, 0);
        }
    }

    pub fn has_focus(&mut self) -> bool {
        let focused_window = unsafe { GetFocus() };
        focused_window == self.state.hwnd
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
        unsafe {
            let cursor = LoadCursorW(null_mut(), cursor_to_lpcwstr(mouse_cursor));
            SetCursor(cursor);
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&GlContext> {
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
