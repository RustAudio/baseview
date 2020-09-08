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
    WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE,
};

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::mpsc;

use raw_window_handle::{windows::WindowsHandle, HasRawWindowHandle, RawWindowHandle};

use crate::{Event, Parent::WithParent, WindowHandler, WindowInfo, WindowOpenOptions};

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
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        PostMessageA(hwnd, WM_SHOWWINDOW, 0, 0);
        return 0;
    }

    let win_ptr = GetWindowLongPtrA(hwnd, GWLP_USERDATA) as *const c_void;
    if !win_ptr.is_null() {
        let window_state = &*(win_ptr as *const RefCell<WindowState<H>>);
        let mut window = Window { hwnd };

        match msg {
            WM_MOUSEMOVE => {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;
                window_state
                    .borrow_mut()
                    .handler
                    .on_event(&mut window, Event::CursorMotion(x, y));
                return 0;
            }
            WM_TIMER => {
                handle_timer(&window_state, wparam);
                return 0;
            }
            WM_PAINT => {
                return 0;
            }
            WM_CLOSE => {
                window_state
                    .borrow_mut()
                    .handler
                    .on_event(&mut window, Event::WillClose);
                return DefWindowProcA(hwnd, msg, wparam, lparam);
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
    scaling: Option<f64>, // DPI scale, 96.0 is "default".
    handler: H,
}

pub struct Window {
    hwnd: HWND,
}

impl Window {
    pub fn open<H: WindowHandler>(
        options: WindowOpenOptions,
        app_message_rx: mpsc::Receiver<H::Message>,
    ) {
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

            let mut rect = RECT {
                left: 0,
                top: 0,
                // todo: check if usize fits into i32
                right: options.width as i32,
                bottom: options.height as i32,
            };

            // todo: add check flags https://github.com/wrl/rutabaga/blob/f30ff67e157375cafdbafe5fb549f1790443a3a8/src/platform/win/window.c#L351
            let mut parent = null_mut();
            if let WithParent(p) = options.parent {
                parent = p;
                flags = WS_CHILD | WS_VISIBLE;
            } else {
                AdjustWindowRectEx(&mut rect, flags, FALSE, 0);
            }

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

            let mut window = Window { hwnd };

            let handler = H::build(&mut window);

            let window_state = Rc::new(RefCell::new(WindowState {
                window_class,
                scaling: None,
                handler,
            }));

            let win = Rc::new(RefCell::new(window));

            SetWindowLongPtrA(hwnd, GWLP_USERDATA, Rc::into_raw(win) as *const _ as _);

            SetTimer(hwnd, 4242, 13, None);

            // todo: decide what to do with the message pump
            if parent.is_null() {
                let mut msg: MSG = std::mem::zeroed();
                loop {
                    let status = GetMessageA(&mut msg, hwnd, 0, 0);
                    if status == -1 {
                        break;
                    }
                    TranslateMessage(&mut msg);
                    DispatchMessageA(&mut msg);
                }
            }
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
