use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{ATOM, FALSE, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::combaseapi::CoCreateGuid;
use winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExA, DefWindowProcA, DestroyWindow, DispatchMessageA,
    GetMessageA, GetWindowLongPtrA, MessageBoxA, PostMessageA, RegisterClassA, SetTimer,
    SetWindowLongPtrA, TranslateMessage, UnregisterClassA, CS_OWNDC, GWLP_USERDATA, MB_ICONERROR,
    MB_OK, MB_TOPMOST, MSG, WM_CLOSE, WM_CREATE, WM_MOUSEMOVE, WM_PAINT, WM_SHOWWINDOW, WM_TIMER,
    WNDCLASSA, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE, WM_DPICHANGED, WM_CHAR, WM_SYSCHAR, WM_KEYDOWN,
    WM_SYSKEYDOWN, WM_KEYUP, WM_SYSKEYUP, WM_INPUTLANGCHANGE,
    GET_XBUTTON_WPARAM, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
};

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::rc::Rc;

use raw_window_handle::{
    windows::WindowsHandle,
    HasRawWindowHandle,
    RawWindowHandle
};

use crate::{
    Event, MouseButton, MouseEvent, Parent::WithParent, ScrollDelta, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy, Size, Point, PhySize, PhyPoint,
};

use super::keyboard::KeyboardState;


unsafe fn message_box(title: &str, msg: &str) {
    let title = (title.to_owned() + "\0").as_ptr() as *const i8;
    let msg = (msg.to_owned() + "\0").as_ptr() as *const i8;
    MessageBoxA(null_mut(), msg, title, MB_ICONERROR | MB_OK | MB_TOPMOST);
}

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

unsafe fn handle_timer<H: WindowHandler>(window_state: &RefCell<WindowState<H>>, timer_id: usize) {
    match timer_id {
        WIN_FRAME_TIMER => {}
        _ => (),
    }
}

unsafe extern "system" fn wnd_proc<H: WindowHandler>(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        PostMessageA(hwnd, WM_SHOWWINDOW, 0, 0);
        return 0;
    }

    let win_ptr = GetWindowLongPtrA(hwnd, GWLP_USERDATA) as *const c_void;
    if !win_ptr.is_null() {
        let window_state = &*(win_ptr as *const RefCell<WindowState<H>>);
        let mut window = Window { hwnd };
        let mut window = crate::Window(&mut window);

        match msg {
            WM_MOUSEMOVE => {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;

                let physical_pos = PhyPoint { x, y };

                let mut window_state = window_state.borrow_mut();

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
                            MouseEvent::ButtonPressed(button)
                        }
                        WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP | WM_XBUTTONUP => {
                            MouseEvent::ButtonReleased(button)
                        }
                        _ => {
                            unreachable!()
                        }
                    };

                    window_state.borrow_mut()
                        .handler
                        .on_event(&mut window, Event::Mouse(event));
                }
            }
            WM_TIMER => {
                handle_timer(&window_state, wparam);
                return 0;
            }
            WM_CLOSE => {
                window_state
                    .borrow_mut()
                    .handler
                    .on_event(&mut window, Event::Window(WindowEvent::WillClose));
                return DefWindowProcA(hwnd, msg, wparam, lparam);
            }
            WM_DPICHANGED => {
                // TODO: Notify app of DPI change
            },
            WM_CHAR | WM_SYSCHAR | WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP
            | WM_SYSKEYUP | WM_INPUTLANGCHANGE => {
                // Will swallow menu key events. See druid code for how to
                // solve that
                let opt_event = window_state.borrow_mut()
                    .keyboard_state
                    .process_message(hwnd, msg, wparam, lparam);

                if let Some(event) = opt_event {
                    window_state.borrow_mut()
                        .handler
                        .on_event(&mut window, Event::Keyboard(event));
                }

                return 0;
            }
            _ => {}
        }
    }

    return DefWindowProcA(hwnd, msg, wparam, lparam);
}

unsafe fn register_wnd_class<H: WindowHandler>() -> ATOM {
    // We generate a unique name for the new window class to prevent name collisions
    let class_name = format!("Baseview-{}", generate_guid()).as_ptr() as *const i8;

    let wnd_class = WNDCLASSA {
        style: CS_OWNDC,
        lpfnWndProc: Some(wnd_proc::<H>),
        hInstance: null_mut(),
        lpszClassName: class_name,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null_mut(),
    };

    RegisterClassA(&wnd_class)
}

unsafe fn unregister_wnd_class(wnd_class: ATOM) {
    UnregisterClassA(wnd_class as _, null_mut());
}

struct WindowState<H> {
    window_class: ATOM,
    window_info: WindowInfo,
    keyboard_state: KeyboardState,
    handler: H,
}

pub struct Window {
    hwnd: HWND,
}

pub struct WindowHandle<H: WindowHandler> {
    // FIXME: replace this with channel sender
    phantom_data: std::marker::PhantomData<H::Message>,
}

impl <H: WindowHandler>WindowHandle<H> {
    pub fn try_send_message(
        &mut self,
        message: H::Message
    ) -> Result<(), H::Message> {
        Err(message)
    }
}

pub struct AppRunner {
    hwnd: HWND,
}

impl AppRunner {
    pub fn app_run_blocking(self) {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();

            loop {
                let status = GetMessageA(&mut msg, self.hwnd, 0, 0);

                if status == -1 {
                    break;
                }

                TranslateMessage(&mut msg);
                DispatchMessageA(&mut msg);
            }
        }
    }
}

impl Window {
    pub fn open<H, B>(
        options: WindowOpenOptions,
        build: B
    ) -> (crate::WindowHandle<H>, Option<crate::AppRunner>)
        where H: WindowHandler,
              B: FnOnce(&mut crate::Window) -> H,
              B: Send + 'static
    {
        unsafe {
            let title = (options.title.to_owned() + "\0").as_ptr() as *const i8;

            let window_class = register_wnd_class::<H>();
            // todo: manage error ^

            let mut flags = WS_POPUPWINDOW
                | WS_CAPTION
                | WS_VISIBLE
                | WS_SIZEBOX
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_CLIPSIBLINGS;
            
            let scaling = match options.scale {
                WindowScalePolicy::SystemScaleFactor => get_scaling().unwrap_or(1.0),
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

            // todo: add check flags https://github.com/wrl/rutabaga/blob/f30ff67e157375cafdbafe5fb549f1790443a3a8/src/platform/win/window.c#L351
            let parent = match options.parent {
                WithParent(RawWindowHandle::Windows(h)) => {
                    flags = WS_CHILD | WS_VISIBLE;
                    h.hwnd
                }

                WithParent(h) => panic!("unsupported parent handle {:?}", h),

                _ => {
                    AdjustWindowRectEx(&mut rect, flags, FALSE, 0);
                    null_mut()
                }
            };

            let hwnd = CreateWindowExA(
                0,
                window_class as _,
                title,
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

            let handler = build(&mut crate::Window(&mut Window { hwnd }));

            let window_state = Box::new(RefCell::new(WindowState {
                window_class,
                window_info,
                keyboard_state: KeyboardState::new(),
                handler,
            }));

            SetWindowLongPtrA(hwnd, GWLP_USERDATA, Box::into_raw(window_state) as *const _ as _);
            SetTimer(hwnd, 4242, 13, None);

            let window_handle = crate::WindowHandle(WindowHandle {
                phantom_data: std::marker::PhantomData,
            });

            let opt_app_runner = if let crate::Parent::None = options.parent {
                Some(crate::AppRunner(AppRunner { hwnd }))
            } else {
                None
            };

            (window_handle, opt_app_runner)
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

fn get_scaling() -> Option<f64> {
    // TODO: find system scaling
    None
}