use winapi::Interface;
use winapi::shared::guiddef::{GUID, REFIID, IsEqualIID};
use winapi::shared::minwindef::{ATOM, FALSE, LPARAM, LRESULT, UINT, WPARAM, DWORD};
use winapi::shared::ntdef::{HRESULT, ULONG};
use winapi::shared::windef::{HWND, RECT, POINTL};
use winapi::shared::winerror::{S_OK, E_NOINTERFACE};
use winapi::shared::wtypes::DVASPECT_CONTENT;
use winapi::um::combaseapi::CoCreateGuid;
use winapi::um::objidl::{IDataObject, STGMEDIUM, FORMATETC, TYMED_HGLOBAL};
use winapi::um::ole2::{RegisterDragDrop, OleInitialize};
use winapi::um::oleidl::{IDropTarget, IDropTargetVtbl, LPDROPTARGET, DROPEFFECT_COPY, DROPEFFECT_NONE, DROPEFFECT_MOVE, DROPEFFECT_LINK, DROPEFFECT_SCROLL};
use winapi::um::shellapi::DragQueryFileW;
use winapi::um::unknwnbase::{IUnknownVtbl, IUnknown};
use winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetDpiForWindow, GetMessageW, GetWindowLongPtrW, LoadCursorW, PostMessageW, RegisterClassW,
    ReleaseCapture, SetCapture, SetProcessDpiAwarenessContext, SetTimer, SetWindowLongPtrW,
    SetWindowPos, TranslateMessage, UnregisterClassW, CS_OWNDC, GET_XBUTTON_WPARAM, GWLP_USERDATA,
    IDC_ARROW, MSG, SWP_NOMOVE, SWP_NOZORDER, WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_CREATE,
    WM_DPICHANGED, WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SHOWWINDOW, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_TIMER, WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE,
    XBUTTON1, XBUTTON2, CF_HDROP,
};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{c_void, OsStr, OsString};
use std::marker::PhantomData;
use std::mem::transmute;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::prelude::OsStringExt;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::Arc;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, Win32Handle};

const BV_WINDOW_MUST_CLOSE: UINT = WM_USER + 1;

use crate::{
    Event, MouseButton, MouseEvent, PhyPoint, PhySize, ScrollDelta, Size, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy, DropEffect, EventStatus, DropData, Point,
};

use super::keyboard::KeyboardState;

#[cfg(feature = "opengl")]
use crate::{gl::GlContext, window::RawWindowHandleWrapper};

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

    // Ensure handle is !Send
    _phantom: PhantomData<*mut ()>,
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
            let mut handle = Win32Handle::empty();
            handle.hwnd = hwnd as *mut c_void;

            RawWindowHandle::Win32(handle)
        } else {
            RawWindowHandle::Win32(Win32Handle::empty())
        }
    }
}

struct ParentHandle {
    is_open: Rc<Cell<bool>>,
}

impl ParentHandle {
    pub fn new(hwnd: HWND) -> (Self, WindowHandle) {
        let is_open = Rc::new(Cell::new(true));

        let handle = WindowHandle {
            hwnd: Some(hwnd),
            is_open: Rc::clone(&is_open),
            _phantom: PhantomData::default(),
        };

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
            unregister_wnd_class((*window_state_ptr).window_class);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(window_state_ptr));
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
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);

            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let physical_pos = PhyPoint { x, y };
            let logical_pos = physical_pos.to_logical(&window_state.window_info.borrow());
            let event = Event::Mouse(MouseEvent::CursorMoved {
                position: logical_pos,
                modifiers: window_state
                    .keyboard_state
                    .borrow()
                    .get_modifiers_from_mouse_wparam(wparam),
            });

            window_state.handler.borrow_mut().as_mut().unwrap().on_event(&mut window, event);

            Some(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);

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
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);

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
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);

            if wparam == WIN_FRAME_TIMER {
                window_state.handler.borrow_mut().as_mut().unwrap().on_frame(&mut window);
            }

            Some(0)
        }
        WM_CLOSE => {
            // Make sure to release the borrow before the DefWindowProc call
            {
                let mut window = window_state.create_window();
                let mut window = crate::Window::new(&mut window);

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
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);

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
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);

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
struct WindowState {
    /// The HWND belonging to this window. The window's actual state is stored in the `WindowState`
    /// struct associated with this HWND through `unsafe { GetWindowLongPtrW(self.hwnd,
    /// GWLP_USERDATA) } as *const WindowState`.
    pub hwnd: HWND,
    window_class: ATOM,
    window_info: RefCell<WindowInfo>,
    _parent_handle: Option<ParentHandle>,
    keyboard_state: RefCell<KeyboardState>,
    mouse_button_counter: Cell<usize>,
    // Initialized late so the `Window` can hold a reference to this `WindowState`
    handler: RefCell<Option<Box<dyn WindowHandler>>>,
    scale_policy: WindowScalePolicy,
    dw_style: u32,
    _drop_target: Option<Arc<DropTarget>>,

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
    fn create_window(&self) -> Window {
        Window { state: self }
    }

    /// Handle a deferred task as described in [`Self::deferred_tasks
    pub(self) fn handle_deferred_task(&self, task: WindowTask) {
        match task {
            WindowTask::Resize(size) => {
                let window_info = {
                    let mut window_info = self.window_info.borrow_mut();
                    let scaling = window_info.scale();
                    *window_info = WindowInfo::from_logical_size(size, scaling);

                    *window_info
                };

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
enum WindowTask {
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

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (window_handle, _) = Self::open(true, null_mut(), options, build);

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
                let mut handle = Win32Handle::empty();
                handle.hwnd = hwnd as *mut c_void;
                let handle = RawWindowHandleWrapper { handle: RawWindowHandle::Win32(handle) };

                GlContext::create(&handle, gl_config).expect("Could not create OpenGL context")
            });

            let (parent_handle, window_handle) = ParentHandle::new(hwnd);
            let parent_handle = if parented { Some(parent_handle) } else { None };

            let window_state = Box::new(WindowState {
                hwnd,
                window_class,
                window_info: RefCell::new(window_info),
                _parent_handle: parent_handle,
                keyboard_state: RefCell::new(KeyboardState::new()),
                mouse_button_counter: Cell::new(0),
                // The Window refers to this `WindowState`, so this `handler` needs to be
                // initialized later
                handler: RefCell::new(None),
                scale_policy: options.scale,
                dw_style: flags,
                _drop_target: None,

                deferred_tasks: RefCell::new(VecDeque::with_capacity(4)),

                #[cfg(feature = "opengl")]
                gl_context,
            });

            let handler = {
                let mut window = window_state.create_window();
                let mut window = crate::Window::new(&mut window);

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

            let window_state_ptr = Box::into_raw(window_state);
            let drop_target = Arc::new(DropTarget::new(window_state_ptr));

            OleInitialize(null_mut());
            RegisterDragDrop(hwnd, Arc::as_ptr(&drop_target) as LPDROPTARGET);

            (*window_state_ptr)._drop_target = Some(drop_target);

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_state_ptr as *const _ as _);
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

    pub fn resize(&mut self, size: Size) {
        // To avoid reentrant event handler calls we'll defer the actual resizing until after the
        // event has been handled
        let task = WindowTask::Resize(size);
        self.state.deferred_tasks.borrow_mut().push_back(task);
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&GlContext> {
        self.state.gl_context.as_ref()
    }
}

unsafe impl HasRawWindowHandle for Window<'_> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32Handle::empty();
        handle.hwnd = self.state.hwnd as *mut c_void;

        RawWindowHandle::Win32(handle)
    }
}

pub fn copy_to_clipboard(data: &str) {
    todo!()
}

#[repr(C)]
pub struct DropTarget {
    base: IDropTarget,
    vtbl: Arc<IDropTargetVtbl>,

    window_state: *mut WindowState,

    // These are cached since DragOver and DragLeave callbacks don't provide them,
    // and handling drag move events gets awkward on the client end otherwise
    drag_position: Point,
    drop_data: DropData,
}

impl DropTarget {
    fn new(window_state: *mut WindowState) -> Self {
        let vtbl = Arc::new(IDropTargetVtbl {
            parent: IUnknownVtbl {
                QueryInterface: Self::query_interface,
                AddRef: Self::add_ref,
                Release: Self::release,
            },
            DragEnter: Self::drag_enter,
            DragOver: Self::drag_over,
            DragLeave: Self::drag_leave,
            Drop: Self::drop,
        });
       
        Self {
            base: IDropTarget { lpVtbl: Arc::as_ptr(&vtbl) },
            vtbl,

            window_state,

            drag_position: Point::new(0.0, 0.0),
            drop_data: DropData::None,
        }
    }

    fn on_event(&self, pdwEffect: Option<*mut DWORD>, event: MouseEvent) {
        unsafe {
            let window_state = &*self.window_state;
            let mut window = window_state.create_window();
            let mut window = crate::Window::new(&mut window);
    
            let event = Event::Mouse(event);
            let event_status = window_state.handler.borrow_mut().as_mut().unwrap().on_event(&mut window, event);

            if let Some(pdwEffect) = pdwEffect {
                match event_status {
                    EventStatus::AcceptDrop(DropEffect::Copy) => *pdwEffect = DROPEFFECT_COPY,
                    EventStatus::AcceptDrop(DropEffect::Move) => *pdwEffect = DROPEFFECT_MOVE,
                    EventStatus::AcceptDrop(DropEffect::Link) => *pdwEffect = DROPEFFECT_LINK,
                    EventStatus::AcceptDrop(DropEffect::Scroll) => *pdwEffect = DROPEFFECT_SCROLL,
                    _ => *pdwEffect = DROPEFFECT_NONE,
                }        
            } 
        }
    }

    fn parse_coordinates(&mut self, pt: *const POINTL) {
        // There's a bug in winapi: the POINTL pointer should actually be a POINTL structure
        // This happens to work on 64-bit platforms because two c_longs (that translate to
        // 32-bit signed integers) happen to be the same size as a 64-bit pointer...
        // For now, just hack around that bug
        let window_state = unsafe { &*self.window_state };

        let x = pt as i64 & u32::MAX as i64;
        let y = pt as i64 >> 32;
        
        let phy_point = PhyPoint::new(x as i32, y as i32);
        self.drag_position = phy_point.to_logical(&window_state.window_info.borrow())
    }

    fn parse_drop_data(&mut self, data_object: &IDataObject) {
        let format = FORMATETC {
            cfFormat: CF_HDROP as u16,
            ptd: null_mut(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL,
        };

        let mut medium = STGMEDIUM {
            tymed: 0,
            u: null_mut(),
            pUnkForRelease: null_mut(),
        };

        unsafe {
            let hresult = data_object.GetData(&format, &mut medium);
            if hresult != S_OK {
                self.drop_data = DropData::None;
                return;
            }

            let hdrop = transmute((*medium.u).hGlobal());
       
            let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, null_mut(), 0);
            if item_count == 0 {
                self.drop_data = DropData::None;
                return;
            }
            
            let mut paths = Vec::with_capacity(item_count as usize);

            for i in 0..item_count {
                let characters = DragQueryFileW(hdrop, i, null_mut(), 0);
                let buffer_size = characters as usize + 1;
                let mut buffer = Vec::<u16>::with_capacity(buffer_size);

                DragQueryFileW(hdrop, i, transmute(buffer.spare_capacity_mut().as_mut_ptr()), buffer_size as u32);
                buffer.set_len(buffer_size);

                paths.push(OsString::from_wide(&buffer[..characters as usize]).into())
            }

            self.drop_data = DropData::Files(paths);
        }
    }

    unsafe extern "system" fn query_interface(
        this: *mut IUnknown,
        riid: REFIID,
        ppvObject: *mut *mut winapi::ctypes::c_void,
    ) -> HRESULT
    {
        if IsEqualIID(&*riid, &IUnknown::uuidof()) || IsEqualIID(&*riid, &IDropTarget::uuidof()){
            Self::add_ref(this);
            *ppvObject = unsafe { transmute(this) };
            return S_OK;
        }
    
        return E_NOINTERFACE;
    }
    
    unsafe extern "system" fn add_ref(this: *mut IUnknown) -> ULONG {
        let arc = Arc::from_raw(this);
        let result = Arc::strong_count(&arc) + 1;
        let _ = Arc::into_raw(arc);

        Arc::increment_strong_count(this);

        result as ULONG
    }
    
    unsafe extern "system" fn release(this: *mut IUnknown) -> ULONG {
        let arc = Arc::from_raw(this);
        let result = Arc::strong_count(&arc) - 1;
        let _ = Arc::into_raw(arc);

        Arc::decrement_strong_count(this);

        result as ULONG
    }
        
    unsafe extern "system" fn drag_enter(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: DWORD,
        pt: *const POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT
    {
        let drop_target = &mut *(this as *mut DropTarget);
        let window_state = unsafe { &*drop_target.window_state };
        
        drop_target.parse_coordinates(pt);
        drop_target.parse_drop_data(&*pDataObj);

        let event = MouseEvent::DragEntered {
            position: drop_target.drag_position,
            modifiers: window_state
                .keyboard_state
                .borrow()
                .get_modifiers_from_mouse_wparam(grfKeyState as WPARAM),
            data: drop_target.drop_data.clone(),
        };

        drop_target.on_event(Some(pdwEffect), event);
        S_OK
    }
    
    unsafe extern "system" fn drag_over(
        this: *mut IDropTarget,
        grfKeyState: DWORD,
        pt: *const POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT
    {
        let drop_target = &mut *(this as *mut DropTarget);
        let window_state = unsafe { &*drop_target.window_state };

        drop_target.parse_coordinates(pt);

        let event = MouseEvent::DragMoved {
            position: drop_target.drag_position,
            modifiers: window_state
                .keyboard_state
                .borrow()
                .get_modifiers_from_mouse_wparam(grfKeyState as WPARAM),
            data: drop_target.drop_data.clone(),
        };

        drop_target.on_event(Some(pdwEffect), event);
        S_OK
    }
    
    unsafe extern "system" fn drag_leave(this: *mut IDropTarget) -> HRESULT {
        let drop_target = &mut *(this as *mut DropTarget);
        drop_target.on_event(None, MouseEvent::DragLeft);
        S_OK
    }
    
    unsafe extern "system" fn drop(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: DWORD,
        pt: *const POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT
    {
        let drop_target = &mut *(this as *mut DropTarget);
        let window_state = unsafe { &*drop_target.window_state };

        drop_target.parse_coordinates(pt);
        drop_target.parse_drop_data(&*pDataObj);

        let event = MouseEvent::DragDropped {
            position: drop_target.drag_position,
            modifiers: window_state
                .keyboard_state
                .borrow()
                .get_modifiers_from_mouse_wparam(grfKeyState as WPARAM),
            data: drop_target.drop_data.clone(),
        };

        drop_target.on_event(Some(pdwEffect), event);
        S_OK
    }
}
