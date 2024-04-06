use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{ATOM, LOWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::combaseapi::CoCreateGuid;
use winapi::um::ole2::{OleInitialize, RegisterDragDrop, RevokeDragDrop};
use winapi::um::oleidl::LPDROPTARGET;
use winapi::um::winuser::{
    DefWindowProcW, DestroyWindow, DispatchMessageW, GetFocus, GetMessageW, GetWindowLongPtrW,
    LoadCursorW, PostMessageW, RegisterClassW, ReleaseCapture, SetCapture, SetCursor, SetFocus,
    SetProcessDpiAwarenessContext, SetTimer, SetWindowLongPtrW, TrackMouseEvent, TranslateMessage,
    UnregisterClassW, CS_OWNDC, GET_XBUTTON_WPARAM, GWLP_USERDATA, HTCLIENT, IDC_ARROW, MSG,
    TRACKMOUSEEVENT, USER_DEFAULT_SCREEN_DPI, WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_CREATE,
    WM_DPICHANGED, WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_NCDESTROY, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SHOWWINDOW, WM_SIZE, WM_SYSCHAR,
    WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSW,
    XBUTTON1, XBUTTON2,
};

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::VecDeque;
use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::rc::Rc;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};

const BV_WINDOW_MUST_CLOSE: UINT = WM_USER + 1;

use crate::{
    Event, MouseButton, MouseCursor, MouseEvent, PhyPoint, PhySize, ScrollDelta, Size, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};

use super::cursor::cursor_to_lpcwstr;
use super::drop_target::DropTarget;
use super::keyboard::KeyboardState;

#[cfg(feature = "opengl")]
use crate::gl::GlContext;
use crate::win::win32_window::Win32Window;

unsafe fn generate_guid() -> String {
    let mut guid: GUID = std::mem::zeroed();
    CoCreateGuid(&mut guid);
    format!(
        "{:0X}-{:0X}-{:0X}-{:0X}{:0X}-{:0X}{:0X}{:0X}{:0X}{:0X}{:0X}\0",
        guid.Data1,
        guid.Data2,
        guid.Data3,
        guid.Data4[0],
        guid.Data4[1],
        guid.Data4[2],
        guid.Data4[3],
        guid.Data4[4],
        guid.Data4[5],
        guid.Data4[6],
        guid.Data4[7]
    )
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
            handle.hwnd = hwnd as *mut c_void;

            RawWindowHandle::Win32(handle)
        } else {
            RawWindowHandle::Win32(Win32WindowHandle::empty())
        }
    }
}

struct ParentHandle {
    is_open: Rc<Cell<bool>>,
}

impl ParentHandle {
    pub fn new(hwnd: HWND) -> (Self, WindowHandle) {
        let is_open = Rc::new(Cell::new(true));

        let handle = WindowHandle { hwnd: Some(hwnd), is_open: Rc::clone(&is_open) };

        (Self { is_open }, handle)
    }
}

impl Drop for ParentHandle {
    fn drop(&mut self) {
        self.is_open.set(false);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        PostMessageW(hwnd, WM_SHOWWINDOW, 0, 0);
        return 0;
    }

    let window_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    if !window_state_ptr.is_null() {
        let result = wnd_proc_inner(hwnd, msg, wparam, lparam, &*window_state_ptr);

        // If any of the above event handlers caused tasks to be pushed to the deferred tasks list,
        // then we'll try to handle them now
        loop {
            // NOTE: This is written like this instead of using a `while let` loop to avoid exending
            //       the borrow of `window_state.deferred_tasks` into the call of
            //       `window_state.handle_deferred_task()` since that may also generate additional
            //       messages.
            let task = match (*window_state_ptr).deferred_tasks.borrow_mut().pop_front() {
                Some(task) => task,
                None => break,
            };

            (*window_state_ptr).handle_deferred_task(task);
        }

        // NOTE: This is not handled in `wnd_proc_inner` because of the deferred task loop above
        if msg == WM_NCDESTROY {
            RevokeDragDrop(hwnd);
            unregister_wnd_class((*window_state_ptr).window_class);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Rc::from_raw(window_state_ptr));
        }

        // The actual custom window proc has been moved to another function so we can always handle
        // the deferred tasks regardless of whether the custom window proc returns early or not
        if let Some(result) = result {
            return result;
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Our custom `wnd_proc` handler. If the result contains a value, then this is returned after
/// handling any deferred tasks. otherwise the default window procedure is invoked.
unsafe fn wnd_proc_inner(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM, window_state: &WindowState,
) -> Option<LRESULT> {
    match msg {
        WM_MOUSEMOVE => {
            let mut window = crate::Window::new(window_state.create_window());

            let mut mouse_was_outside_window = window_state.mouse_was_outside_window.borrow_mut();
            if *mouse_was_outside_window {
                // this makes Windows track whether the mouse leaves the window.
                // When the mouse leaves it results in a `WM_MOUSELEAVE` event.
                let mut track_mouse = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: winapi::um::winuser::TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: winapi::um::winuser::HOVER_DEFAULT,
                };
                // Couldn't find a good way to track whether the mouse enters,
                // but if `WM_MOUSEMOVE` happens, the mouse must have entered.
                TrackMouseEvent(&mut track_mouse);
                *mouse_was_outside_window = false;

                let enter_event = Event::Mouse(MouseEvent::CursorEntered);
                window_state
                    .handler
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .on_event(&mut window, enter_event);
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
            window_state.handler.borrow_mut().as_mut().unwrap().on_event(&mut window, move_event);
            Some(0)
        }

        WM_MOUSELEAVE => {
            let mut window = crate::Window::new(window_state.create_window());
            let event = Event::Mouse(MouseEvent::CursorLeft);
            window_state.handler.borrow_mut().as_mut().unwrap().on_event(&mut window, event);

            *window_state.mouse_was_outside_window.borrow_mut() = true;
            Some(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let mut window = crate::Window::new(window_state.create_window());

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

            window_state.handler.borrow_mut().as_mut().unwrap().on_event(&mut window, event);

            Some(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_RBUTTONDOWN
        | WM_RBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let mut window = crate::Window::new(window_state.create_window());

            let mut mouse_button_counter = window_state.mouse_button_counter.get();

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

                window_state
                    .handler
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .on_event(&mut window, Event::Mouse(event));
            }

            None
        }
        WM_TIMER => {
            let mut window = crate::Window::new(window_state.create_window());

            if wparam == WIN_FRAME_TIMER {
                window_state.handler.borrow_mut().as_mut().unwrap().on_frame(&mut window);
            }

            Some(0)
        }
        WM_CLOSE => {
            // Make sure to release the borrow before the DefWindowProc call
            {
                let mut window = crate::Window::new(window_state.create_window());

                window_state
                    .handler
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .on_event(&mut window, Event::Window(WindowEvent::WillClose));
            }

            // DestroyWindow(hwnd);
            // Some(0)
            Some(DefWindowProcW(hwnd, msg, wparam, lparam))
        }
        WM_CHAR | WM_SYSCHAR | WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
        | WM_INPUTLANGCHANGE => {
            let mut window = crate::Window::new(window_state.create_window());

            let opt_event =
                window_state.keyboard_state.borrow_mut().process_message(hwnd, msg, wparam, lparam);

            if let Some(event) = opt_event {
                window_state
                    .handler
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .on_event(&mut window, Event::Keyboard(event));
            }

            if msg != WM_SYSKEYDOWN {
                Some(0)
            } else {
                None
            }
        }
        WM_SIZE => {
            let new_physical_size = PhySize {
                width: (lparam & 0xFFFF) as u16 as u32,
                height: ((lparam >> 16) & 0xFFFF) as u16 as u32,
            };

            // Only send the event if anything changed
            if new_physical_size == window_state.current_size.get() {
                return None;
            }

            window_state.current_size.set(new_physical_size);

            let mut window = crate::Window::new(window_state.create_window());
            let new_size = WindowInfo::from_physical_size(
                new_physical_size,
                window_state.current_scale_factor.get(),
            );

            window_state
                .handler
                .borrow_mut()
                .as_mut()
                .unwrap()
                .on_event(&mut window, Event::Window(WindowEvent::Resized(new_size)));

            None
        }
        WM_DPICHANGED => {
            let dpi = (wparam & 0xFFFF) as u16 as u32;
            let suggested_rect = &*(lparam as *const RECT);

            let new_scale_factor = dpi as f64 / USER_DEFAULT_SCREEN_DPI as f64;
            window_state.set_new_scale_factor(new_scale_factor, Some(suggested_rect));

            None
        }
        // If WM_SETCURSOR returns `None`, WM_SETCURSOR continues to get handled by the outer window(s),
        // If it returns `Some(1)`, the current window decides what the cursor is
        WM_SETCURSOR => {
            let low_word = LOWORD(lparam as u32) as isize;
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

unsafe fn register_wnd_class() -> ATOM {
    // We generate a unique name for the new window class to prevent name collisions
    let class_name_str = format!("Baseview-{}", generate_guid());
    let mut class_name: Vec<u16> = OsStr::new(&class_name_str).encode_wide().collect();
    class_name.push(0);

    let wnd_class = WNDCLASSW {
        style: CS_OWNDC,
        lpfnWndProc: Some(wnd_proc),
        hInstance: null_mut(),
        lpszClassName: class_name.as_ptr(),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hIcon: null_mut(),
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: null_mut(),
        lpszMenuName: null_mut(),
    };

    RegisterClassW(&wnd_class)
}

unsafe fn unregister_wnd_class(wnd_class: ATOM) {
    UnregisterClassW(wnd_class as _, null_mut());
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
    pub window: Win32Window,
    window_class: ATOM,
    current_size: Cell<PhySize>,
    current_scale_factor: Cell<f64>,
    _parent_handle: Option<ParentHandle>,
    keyboard_state: RefCell<KeyboardState>,
    mouse_button_counter: Cell<usize>,
    mouse_was_outside_window: RefCell<bool>,
    cursor_icon: Cell<MouseCursor>,
    // Initialized late so the `Window` can hold a reference to this `WindowState`
    handler: RefCell<Option<Box<dyn WindowHandler>>>,
    _drop_target: RefCell<Option<Rc<DropTarget>>>,
    scale_policy: WindowScalePolicy,

    /// Tasks that should be executed at the end of `wnd_proc`. This is needed to avoid mutably
    /// borrowing the fields from `WindowState` more than once. For instance, when the window
    /// handler requests a resize in response to a keyboard event, the window state will already be
    /// borrowed in `wnd_proc`. So the `resize()` function below cannot also mutably borrow that
    /// window state at the same time.
    pub deferred_tasks: RefCell<VecDeque<WindowTask>>,

    #[cfg(feature = "opengl")]
    pub gl_context: Option<GlContext>,
}

impl WindowState {
    pub(super) fn create_window(&self) -> Window {
        Window { state: self }
    }

    pub(super) fn window_info(&self) -> WindowInfo {
        WindowInfo::from_physical_size(self.current_size.get(), self.current_scale_factor.get())
    }

    pub(super) fn keyboard_state(&self) -> Ref<KeyboardState> {
        self.keyboard_state.borrow()
    }

    pub(super) fn handler_mut(&self) -> RefMut<Option<Box<dyn WindowHandler>>> {
        self.handler.borrow_mut()
    }

    /// Handle a deferred task as described in [`Self::deferred_tasks`].
    pub(self) fn handle_deferred_task(&self, task: WindowTask) {
        match task {
            WindowTask::Resize(size) => {
                // `self.window_info` will be modified in response to the `WM_SIZE` event that
                // follows the `SetWindowPos()` call
                let scaling = self.current_scale_factor.get();
                let new_size = WindowInfo::from_logical_size(size, scaling);

                self.window.resize(new_size.physical_size());
            }
        }
    }

    fn set_new_scale_factor(&self, new_scale_factor: f64, suggested_dimensions: Option<&RECT>) {
        // We don't care about window DPI changes when using a forced scale factor
        if self.scale_policy != WindowScalePolicy::SystemScaleFactor {
            return;
        }

        let current_scale_factor = self.current_scale_factor.get();

        if new_scale_factor == current_scale_factor {
            return;
        }

        self.current_scale_factor.set(new_scale_factor);

        // Windows makes us resize the window manually. This will trigger another `WM_SIZE` event,
        // which will then set the actual size and send the new scale factor to the handler.
        if let Some(suggested_dimensions) = suggested_dimensions {
            // If we have suggested dimensions (from the DPI changed event), then we use them directly
            self.window.set_raw_pos(suggested_dimensions);
        } else {
            // Otherwise, we figure out the correct dimensions ourselves
            let current_size =
                WindowInfo::from_physical_size(self.current_size.get(), current_scale_factor);

            let new_size =
                WindowInfo::from_logical_size(current_size.logical_size(), new_scale_factor);

            self.window.resize(new_size.physical_size());
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
            RawWindowHandle::Win32(h) => h.hwnd as HWND,
            h => panic!("unsupported parent handle {:?}", h),
        };

        let (window_handle, _) = Self::open(Some(parent), options, build);

        window_handle
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (_, hwnd) = Self::open(None, options, build);

        unsafe {
            let mut msg: MSG = std::mem::zeroed();

            loop {
                let status = GetMessageW(&mut msg, hwnd, 0, 0);

                if status == -1 {
                    break;
                }

                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn open<H, B>(
        parent: Option<HWND>, options: WindowOpenOptions, build: B,
    ) -> (WindowHandle, HWND)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        unsafe {
            let window_class = register_wnd_class();
            // todo: manage error ^

            let initial_scale_factor = match options.scale {
                WindowScalePolicy::SystemScaleFactor => 1.0,
                WindowScalePolicy::ScaleFactor(scale) => scale,
            };

            let initial_size =
                WindowInfo::from_logical_size(options.size, initial_scale_factor).physical_size();

            let raw_window =
                Win32Window::create(window_class, &options.title, initial_size, parent);

            #[cfg(feature = "opengl")]
            let gl_context: Option<GlContext> = options.gl_config.map(|gl_config| {
                let mut handle = Win32WindowHandle::empty();
                handle.hwnd = raw_window.handle as *mut c_void;
                let handle = RawWindowHandle::Win32(handle);

                GlContext::create(&handle, gl_config).expect("Could not create OpenGL context")
            });

            let (parent_handle, window_handle) = ParentHandle::new(raw_window.handle);
            let parent_handle = if parent.is_some() { Some(parent_handle) } else { None };

            let window_state = Rc::new(WindowState {
                window: raw_window,
                window_class,
                current_size: Cell::new(initial_size),
                current_scale_factor: Cell::new(initial_scale_factor),
                _parent_handle: parent_handle,
                keyboard_state: RefCell::new(KeyboardState::new()),
                mouse_button_counter: Cell::new(0),
                mouse_was_outside_window: RefCell::new(true),
                cursor_icon: Cell::new(MouseCursor::Default),
                // The Window refers to this `WindowState`, so this `handler` needs to be
                // initialized later
                handler: RefCell::new(None),
                _drop_target: RefCell::new(None),
                scale_policy: options.scale,

                deferred_tasks: RefCell::new(VecDeque::with_capacity(4)),

                #[cfg(feature = "opengl")]
                gl_context,
            });

            let handler = {
                let mut window = crate::Window::new(window_state.create_window());

                build(&mut window)
            };
            *window_state.handler.borrow_mut() = Some(Box::new(handler));

            // Only works on Windows 10 unfortunately.
            SetProcessDpiAwarenessContext(
                winapi::shared::windef::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
            );

            let drop_target = Rc::new(DropTarget::new(Rc::downgrade(&window_state)));
            *window_state._drop_target.borrow_mut() = Some(drop_target.clone());

            OleInitialize(null_mut());
            RegisterDragDrop(window_state.window.handle, Rc::as_ptr(&drop_target) as LPDROPTARGET);

            SetWindowLongPtrW(
                window_state.window.handle,
                GWLP_USERDATA,
                Rc::into_raw(window_state.clone()) as *const _ as _,
            );
            SetTimer(window_state.window.handle, WIN_FRAME_TIMER, 15, None);

            // Now that the window exists, we can get the actual DPI of the screen it's on.
            window_state.set_new_scale_factor(window_state.window.current_scale_factor(), None);

            (window_handle, window_state.window.handle)
        }
    }

    pub fn close(&mut self) {
        unsafe {
            PostMessageW(self.state.window.handle, BV_WINDOW_MUST_CLOSE, 0, 0);
        }
    }

    pub fn has_focus(&mut self) -> bool {
        let focused_window = unsafe { GetFocus() };
        focused_window == self.state.window.handle
    }

    pub fn focus(&mut self) {
        unsafe {
            SetFocus(self.state.window.handle);
        }
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
        self.state.gl_context.as_ref()
    }
}

unsafe impl HasRawWindowHandle for Window<'_> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = self.state.window.handle as *mut c_void;

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
