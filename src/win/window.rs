extern crate winapi;

use std::ptr::null_mut;
use std::sync::mpsc;

use self::winapi::shared::guiddef::GUID;
use self::winapi::shared::minwindef::{ATOM, FALSE, LPARAM, LRESULT, UINT, WPARAM};
use self::winapi::shared::windef::{HWND, RECT};
use self::winapi::um::combaseapi::CoCreateGuid;
use self::winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExA, DefWindowProcA, DestroyWindow, DispatchMessageA, GetDC,
    GetMessageA, GetWindowLongPtrA, MessageBoxA, PeekMessageA, PostMessageA, RegisterClassA,
    ReleaseDC, SetTimer, SetWindowLongPtrA, TranslateMessage, UnregisterClassA, CS_OWNDC,
    GWLP_USERDATA, MB_ICONERROR, MB_OK, MB_TOPMOST, MSG, PM_REMOVE, WM_CREATE, WM_QUIT,
    WM_SHOWWINDOW, WM_TIMER, WNDCLASSA, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUPWINDOW, WS_SIZEBOX, WS_VISIBLE,
};

use self::winapi::ctypes::c_void;
use crate::Parent::WithParent;
use crate::{handle_message, WindowOpenOptions};
use crate::{AppWindow, Event, RawWindow, WindowInfo};
use std::sync::{Arc, Mutex};

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

unsafe extern "system" fn wnd_proc<A: AppWindow>(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let win_ptr = GetWindowLongPtrA(hwnd, GWLP_USERDATA) as *const c_void;
    match msg {
        WM_CREATE => {
            PostMessageA(hwnd, WM_SHOWWINDOW, 0, 0);
            0
        }
        _ => {
            if !win_ptr.is_null() {
                let win_ref: Arc<Mutex<Window<A>>> =
                    Arc::from_raw(win_ptr as *mut Mutex<Window<A>>);
                let win = Arc::clone(&win_ref);
                let ret = handle_message(win, msg, wparam, lparam);

                // todo: need_reconfigure thing?

                // Needed otherwise it crashes because it drops the userdata
                // We basically need to keep the GWLP_USERDATA fresh between calls of the proc
                // DO NOT REMOVE
                SetWindowLongPtrA(hwnd, GWLP_USERDATA, Arc::into_raw(win_ref) as *const _ as _);

                return ret;
            }

            return DefWindowProcA(hwnd, msg, wparam, lparam);
        }
    }
}

unsafe fn register_wnd_class<A: AppWindow>() -> ATOM {
    // We generate a unique name for the new window class to prevent name collisions
    let class_name = format!("Baseview-{}", generate_guid()).as_ptr() as *const i8;

    let wnd_class = WNDCLASSA {
        style: CS_OWNDC,
        lpfnWndProc: Some(wnd_proc::<A>),
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

pub struct Window<A: AppWindow> {
    pub(crate) hwnd: HWND,
    window_class: ATOM,
    app_window: A,
    app_message_rx: mpsc::Receiver<A::AppMessage>,
    scaling: Option<f64>, // DPI scale, 96.0 is "default".
}

impl<A: AppWindow> Window<A> {
    pub fn open(options: WindowOpenOptions, app_message_rx: mpsc::Receiver<A::AppMessage>) {
        unsafe {
            let title = (options.title.to_owned() + "\0").as_ptr() as *const i8;

            let window_class = register_wnd_class::<A>();
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

            let mut windows_handle = raw_window_handle::windows::WindowsHandle::empty();
            windows_handle.hwnd = hwnd as *mut std::ffi::c_void;

            let raw_window = RawWindow {
                raw_window_handle: raw_window_handle::RawWindowHandle::Windows(windows_handle),
            };

            let window_info = WindowInfo {
                width: options.width as u32,
                height: options.height as u32,
                scale: 1.0,
            };

            let app_window = A::build(raw_window, &window_info);

            let mut window = Window {
                hwnd,
                window_class,
                app_window,
                app_message_rx,
                scaling: None,
            };

            let win = Arc::new(Mutex::new(window));
            let win_p = Arc::clone(&win);

            SetWindowLongPtrA(hwnd, GWLP_USERDATA, Arc::into_raw(win) as *const _ as _);

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
                    handle_message(Arc::clone(&win_p), msg.message, msg.wParam, msg.lParam);
                }
            }
        }
    }

    pub fn close(&mut self) {
        self.app_window.on_event(Event::WillClose);

        // todo: see https://github.com/wrl/rutabaga/blob/f30ff67e157375cafdbafe5fb549f1790443a3a8/src/platform/win/window.c#L402
        unsafe {
            DestroyWindow(self.hwnd);
            unregister_wnd_class(self.window_class);
        }
    }

    pub(crate) fn handle_mouse_motion(&mut self, x: i32, y: i32) {
        self.app_window.on_event(Event::CursorMotion(x, y));
    }
}
