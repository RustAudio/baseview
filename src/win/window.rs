use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{ATOM, FALSE, LOWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::combaseapi::CoCreateGuid;
use winapi::um::ole2::{OleInitialize, RegisterDragDrop, RevokeDragDrop};
use winapi::um::oleidl::LPDROPTARGET;
use winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetDpiForWindow, GetFocus, GetMessageW, GetWindowLongPtrW, LoadCursorW, PostMessageW,
    RegisterClassW, ReleaseCapture, SetCapture, SetCursor, SetFocus, SetProcessDpiAwarenessContext,
    SetTimer, SetWindowLongPtrW, SetWindowPos, TrackMouseEvent, TranslateMessage, UnregisterClassW,
    CS_OWNDC, GET_XBUTTON_WPARAM, GWLP_USERDATA, HTCLIENT, IDC_ARROW, MSG, SWP_NOMOVE,
    SWP_NOZORDER, TRACKMOUSEEVENT, WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_CREATE, WM_DPICHANGED,
    WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SHOWWINDOW, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WM_TIMER, WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE,
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
            let logical_pos = physical_pos.to_logical(&window_state.window_info.borrow());
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
            let mut window = crate::Window::new(window_state.create_window());

            let width = (lparam & 0xFFFF) as u16 as u32;
            let height = ((lparam >> 16) & 0xFFFF) as u16 as u32;

            let new_window_info = {
                let mut window_info = window_state.window_info.borrow_mut();
                let new_window_info =
                    WindowInfo::from_physical_size(PhySize { width, height }, window_info.scale());

                // Only send the event if anything changed
                if window_info.physical_size() == new_window_info.physical_size() {
                    return None;
                }

                *window_info = new_window_info;

                new_window_info
            };

            window_state
                .handler
                .borrow_mut()
                .as_mut()
                .unwrap()
                .on_event(&mut window, Event::Window(WindowEvent::Resized(new_window_info)));

            None
        }
        WM_DPICHANGED => {
            // To avoid weirdness with the realtime borrow checker.
            let new_rect = {
                if let WindowScalePolicy::SystemScaleFactor = window_state.scale_policy {
                    let dpi = (wparam & 0xFFFF) as u16 as u32;
                    let scale_factor = dpi as f64 / 96.0;

                    let mut window_info = window_state.window_info.borrow_mut();
                    *window_info =
                        WindowInfo::from_logical_size(window_info.logical_size(), scale_factor);

                    Some((
                        RECT {
                            left: 0,
                            top: 0,
                            // todo: check if usize fits into i32
                            right: window_info.physical_size().width as i32,
                            bottom: window_info.physical_size().height as i32,
                        },
                        window_state.dw_style,
                    ))
                } else {
                    None
                }
            };
            if let Some((mut new_rect, dw_style)) = new_rect {
                // Convert this desired "client rectangle" size to the actual "window rectangle"
                // size (Because of course you have to do that).
                AdjustWindowRectEx(&mut new_rect, dw_style, 0, 0);

                // Windows makes us resize the window manually. This will trigger another `WM_SIZE` event,
                // which we can then send the user the new scale factor.
                SetWindowPos(
                    hwnd,
                    hwnd,
                    new_rect.left,
                    new_rect.top,
                    new_rect.right - new_rect.left,
                    new_rect.bottom - new_rect.top,
                    SWP_NOZORDER | SWP_NOMOVE,
                );
            }

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
    pub hwnd: HWND,
    window_class: ATOM,
    window_info: RefCell<WindowInfo>,
    _parent_handle: Option<ParentHandle>,
    keyboard_state: RefCell<KeyboardState>,
    mouse_button_counter: Cell<usize>,
    mouse_was_outside_window: RefCell<bool>,
    cursor_icon: Cell<MouseCursor>,
    // Initialized late so the `Window` can hold a reference to this `WindowState`
    handler: RefCell<Option<Box<dyn WindowHandler>>>,
    _drop_target: RefCell<Option<Rc<DropTarget>>>,
    scale_policy: WindowScalePolicy,
    dw_style: u32,

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

    pub(super) fn window_info(&self) -> Ref<WindowInfo> {
        self.window_info.borrow()
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
                let scaling = self.window_info.borrow().scale();
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

        let (window_handle, _) = Self::open(true, parent, options, build);

        window_handle
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (_, hwnd) = Self::open(false, null_mut(), options, build);

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
        parented: bool, parent: HWND, options: WindowOpenOptions, build: B,
    ) -> (WindowHandle, HWND)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        unsafe {
            let mut title: Vec<u16> = OsStr::new(&options.title[..]).encode_wide().collect();
            title.push(0);

            let window_class = register_wnd_class();
            // todo: manage error ^

            let scaling = match options.scale {
                WindowScalePolicy::SystemScaleFactor => 1.0,
                WindowScalePolicy::ScaleFactor(scale) => scale,
            };

            let window_info = WindowInfo::from_logical_size(options.size, scaling);

            let mut rect = RECT {
                left: 0,
                top: 0,
                // todo: check if usize fits into i32
                right: window_info.physical_size().width as i32,
                bottom: window_info.physical_size().height as i32,
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
                AdjustWindowRectEx(&mut rect, flags, FALSE, 0);
            }

            let hwnd = CreateWindowExW(
                0,
                window_class as _,
                title.as_ptr(),
                flags,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                parent as *mut _,
                null_mut(),
                null_mut(),
                null_mut(),
            );
            // todo: manage error ^

            #[cfg(feature = "opengl")]
            let gl_context: Option<GlContext> = options.gl_config.map(|gl_config| {
                let mut handle = Win32WindowHandle::empty();
                handle.hwnd = hwnd as *mut c_void;
                let handle = RawWindowHandle::Win32(handle);

                GlContext::create(&handle, gl_config).expect("Could not create OpenGL context")
            });

            let (parent_handle, window_handle) = ParentHandle::new(hwnd);
            let parent_handle = if parented { Some(parent_handle) } else { None };

            let window_state = Rc::new(WindowState {
                hwnd,
                window_class,
                window_info: RefCell::new(window_info),
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
                dw_style: flags,

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

            // Now we can get the actual dpi of the window.
            let new_rect = if let WindowScalePolicy::SystemScaleFactor = options.scale {
                // Only works on Windows 10 unfortunately.
                let dpi = GetDpiForWindow(hwnd);
                let scale_factor = dpi as f64 / 96.0;

                let mut window_info = window_state.window_info.borrow_mut();
                if window_info.scale() != scale_factor {
                    *window_info =
                        WindowInfo::from_logical_size(window_info.logical_size(), scale_factor);

                    Some(RECT {
                        left: 0,
                        top: 0,
                        // todo: check if usize fits into i32
                        right: window_info.physical_size().width as i32,
                        bottom: window_info.physical_size().height as i32,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let drop_target = Rc::new(DropTarget::new(Rc::downgrade(&window_state)));
            *window_state._drop_target.borrow_mut() = Some(drop_target.clone());

            OleInitialize(null_mut());
            RegisterDragDrop(hwnd, Rc::as_ptr(&drop_target) as LPDROPTARGET);

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Rc::into_raw(window_state) as *const _ as _);
            SetTimer(hwnd, WIN_FRAME_TIMER, 15, None);

            if let Some(mut new_rect) = new_rect {
                // Convert this desired"client rectangle" size to the actual "window rectangle"
                // size (Because of course you have to do that).
                AdjustWindowRectEx(&mut new_rect, flags, 0, 0);

                // Windows makes us resize the window manually. This will trigger another `WM_SIZE` event,
                // which we can then send the user the new scale factor.
                SetWindowPos(
                    hwnd,
                    hwnd,
                    new_rect.left,
                    new_rect.top,
                    new_rect.right - new_rect.left,
                    new_rect.bottom - new_rect.top,
                    SWP_NOZORDER | SWP_NOMOVE,
                );
            }

            (window_handle, hwnd)
        }
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
        unsafe {
            SetFocus(self.state.hwnd);
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
        handle.hwnd = self.state.hwnd as *mut c_void;

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
