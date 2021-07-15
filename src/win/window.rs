use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{ATOM, FALSE, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::combaseapi::CoCreateGuid;
use winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetMessageW, GetWindowLongPtrW, PostMessageW, RegisterClassW, SetTimer,
    SetWindowLongPtrW, TranslateMessage, UnregisterClassW, LoadCursorW,
    DestroyWindow, SetProcessDpiAwarenessContext, SetWindowPos,
    GetDpiForWindow,
    CS_OWNDC, GWLP_USERDATA, IDC_ARROW,
    MSG, WM_CLOSE, WM_CREATE, WM_MOUSEMOVE, WM_SHOWWINDOW, WM_TIMER, WM_NCDESTROY,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE, WM_DPICHANGED, WM_CHAR, WM_SYSCHAR, WM_KEYDOWN,
    WM_SYSKEYDOWN, WM_KEYUP, WM_SYSKEYUP, WM_INPUTLANGCHANGE, WM_SIZE,
    GET_XBUTTON_WPARAM, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
    SetCapture, GetCapture, ReleaseCapture, IsWindow, SWP_NOZORDER, SWP_NOMOVE
};

use std::cell::RefCell;
use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use raw_window_handle::{
    windows::WindowsHandle,
    HasRawWindowHandle,
    RawWindowHandle
};

use crate::{
    Event, MouseButton, MouseEvent, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy, PhyPoint, PhySize
};

use super::keyboard::KeyboardState;

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

unsafe extern "system" fn wnd_proc(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        PostMessageW(hwnd, WM_SHOWWINDOW, 0, 0);
        return 0;
    }

    let window_state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<WindowState>;
    if !window_state_ptr.is_null() {
        let mut window = Window { hwnd };
        let mut window = crate::Window::new(&mut window);

        match msg {
            WM_MOUSEMOVE => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                let physical_pos = PhyPoint { x, y };

                let mut window_state = (&*window_state_ptr).borrow_mut();

                let logical_pos = physical_pos.to_logical(&window_state.window_info);

                window_state.handler.on_event(
                    &mut window,
                    Event::Mouse(MouseEvent::CursorMoved {
                        position: logical_pos,
                    }),
                );
                return 0;
            }
            WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP |
            WM_RBUTTONDOWN | WM_RBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {

                let mut mouse_button_counter = (&*window_state_ptr).borrow().mouse_button_counter;

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
                            MouseEvent::ButtonPressed(button)
                        }
                        WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP | WM_XBUTTONUP => {
                            // Release the mouse cursor capture when all buttons are released
                            mouse_button_counter = mouse_button_counter.saturating_sub(1);
                            if mouse_button_counter == 0 {
                                ReleaseCapture();
                            }
                            
                            MouseEvent::ButtonReleased(button)
                        }
                        _ => {
                            unreachable!()
                        }
                    };

                    (&*window_state_ptr).borrow_mut().mouse_button_counter = mouse_button_counter;

                    (&*window_state_ptr).borrow_mut()
                        .handler
                        .on_event(&mut window, Event::Mouse(event));
                }
            }
            WM_TIMER => {
                match wparam {
                    WIN_FRAME_TIMER => {
                        (&*window_state_ptr).borrow_mut().handler.on_frame(&mut window);
                    }
                    _ => (),
                }
                return 0;
            }
            WM_CLOSE => {
                (&*window_state_ptr)
                    .borrow_mut()
                    .handler
                    .on_event(&mut window, Event::Window(WindowEvent::WillClose));
                // DestroyWindow(hwnd);
                // return 0;
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_CHAR | WM_SYSCHAR | WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP
            | WM_SYSKEYUP | WM_INPUTLANGCHANGE => {
                let opt_event = (&*window_state_ptr).borrow_mut()
                    .keyboard_state
                    .process_message(hwnd, msg, wparam, lparam);

                if let Some(event) = opt_event {
                    (&*window_state_ptr).borrow_mut()
                        .handler
                        .on_event(&mut window, Event::Keyboard(event));
                }

                if msg != WM_SYSKEYDOWN {
                    return 0;
                }
            }
            WM_SIZE => {
                let width = (lparam & 0xFFFF) as u16 as u32;
                let height = ((lparam >> 16) & 0xFFFF) as u16 as u32;

                let mut window_state = (&*window_state_ptr).borrow_mut();

                window_state.window_info = WindowInfo::from_physical_size(
                    PhySize { width, height },
                    window_state.window_info.scale(),
                );

                let window_info = window_state.window_info;

                window_state.handler.on_event(
                    &mut window,
                    Event::Window(WindowEvent::Resized(window_info)),
                );
            }
            WM_DPICHANGED => {
                // To avoid weirdness with the realtime borrow checker.
                let new_rect = {
                    let mut window_state = (&*window_state_ptr).borrow_mut();

                    if let WindowScalePolicy::SystemScaleFactor = window_state.scale_policy {
                        let dpi = (wparam & 0xFFFF) as u16 as u32;
                        let scale_factor = dpi as f64 / 96.0;

                        window_state.window_info = WindowInfo::from_logical_size(
                            window_state.window_info.logical_size(),
                            scale_factor,
                        );

                        Some((
                            RECT {
                                left: 0,
                                top: 0,
                                // todo: check if usize fits into i32
                                right: window_state.window_info.physical_size().width as i32,
                                bottom: window_state.window_info.physical_size().height as i32,
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
                        new_rect.left as i32,
                        new_rect.top as i32,
                        new_rect.right - new_rect.left,
                        new_rect.bottom - new_rect.top,
                        SWP_NOZORDER | SWP_NOMOVE,
                    );
                }
            }
            WM_NCDESTROY => {
                let window_state = Box::from_raw(window_state_ptr);
                unregister_wnd_class(window_state.borrow().window_class);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            _ => {}
        }

        
    }

    return DefWindowProcW(hwnd, msg, wparam, lparam);
}

unsafe fn register_wnd_class() -> ATOM {
    // We generate a unique name for the new window class to prevent name collisions
    let class_name_str = format!("Baseview-{}", generate_guid());
    let mut class_name: Vec<u16> = OsStr::new(&class_name_str)
        .encode_wide()
        .collect();
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

struct WindowState {
    window_class: ATOM,
    window_info: WindowInfo,
    keyboard_state: KeyboardState,
    mouse_button_counter: usize,
    handler: Box<dyn WindowHandler>,
    scale_policy: WindowScalePolicy,
    dw_style: u32,
}

pub struct Window {
    hwnd: HWND,
}

impl Window {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B)
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let parent = match parent.raw_window_handle() {
            RawWindowHandle::Windows(h) => h.hwnd as HWND,
            h => panic!("unsupported parent handle {:?}", h),
        };

        Self::open(true, parent, options, build);
    }

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> RawWindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let hwnd = Self::open(true, null_mut(), options, build);

        RawWindowHandle::Windows(WindowsHandle {
            hwnd: hwnd as *mut std::ffi::c_void,
            ..WindowsHandle::empty()
        })
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let hwnd = Self::open(false, null_mut(), options, build);

        unsafe {
            let mut msg: MSG = std::mem::zeroed();

            loop {
                let status = GetMessageW(&mut msg, hwnd, 0, 0);

                if status == -1 {
                    break;
                }

                TranslateMessage(&mut msg);
                DispatchMessageW(&mut msg);
            }
        }
    }

    fn open<H, B>(
        parented: bool,
        parent: HWND,
        options: WindowOpenOptions,
        build: B
    ) -> HWND
    where H: WindowHandler + 'static,
          B: FnOnce(&mut crate::Window) -> H,
          B: Send + 'static,
    {
        unsafe {
            let mut title: Vec<u16> = OsStr::new(&options.title[..])
                .encode_wide()
                .collect();
            title.push(0);

            let window_class = register_wnd_class();
            // todo: manage error ^
            
            let scaling = match options.scale {
                WindowScalePolicy::SystemScaleFactor => 1.0,
                WindowScalePolicy::ScaleFactor(scale) => scale
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

            let handler = Box::new(build(&mut crate::Window::new(&mut Window { hwnd })));

            let mut window_state = Box::new(RefCell::new(WindowState {
                window_class,
                window_info,
                keyboard_state: KeyboardState::new(),
                mouse_button_counter: 0,
                handler,
                scale_policy: options.scale,
                dw_style: flags,
            }));

            // Only works on Windows 10 unfortunately.
            SetProcessDpiAwarenessContext(
                winapi::shared::windef::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
            );

            // Now we can get the actual dpi of the window.
            let new_rect = if let WindowScalePolicy::SystemScaleFactor = options.scale {
                // Only works on Windows 10 unfortunately.
                let dpi = GetDpiForWindow(hwnd);
                let scale_factor = dpi as f64 / 96.0;

                let mut window_state = window_state.get_mut();
                if window_state.window_info.scale() != scale_factor {
                    window_state.window_info = WindowInfo::from_logical_size(
                        window_state.window_info.logical_size(),
                        scale_factor,
                    );

                    Some(RECT {
                        left: 0,
                        top: 0,
                        // todo: check if usize fits into i32
                        right: window_state.window_info.physical_size().width as i32,
                        bottom: window_state.window_info.physical_size().height as i32,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window_state) as *const _ as _);
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
                    new_rect.left as i32,
                    new_rect.top as i32,
                    new_rect.right - new_rect.left,
                    new_rect.bottom - new_rect.top,
                    SWP_NOZORDER | SWP_NOMOVE,
                );
            }

            hwnd
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Windows(WindowsHandle {
            hwnd: self.hwnd as *mut std::ffi::c_void,
            ..WindowsHandle::empty()
        })
    }
}