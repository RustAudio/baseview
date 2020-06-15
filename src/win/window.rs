extern crate winapi;

use std::ffi::CString;
use std::ptr::null_mut;

use self::winapi::shared::guiddef::GUID;
use self::winapi::shared::minwindef::{ATOM, FALSE, LPARAM, LRESULT, UINT, WPARAM};
use self::winapi::shared::windef::{HDC, HGLRC, HWND, RECT};
use self::winapi::um::combaseapi::CoCreateGuid;
use self::winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};
use self::winapi::um::wingdi::{
    wglCreateContext, wglDeleteContext, wglMakeCurrent, ChoosePixelFormat, SetPixelFormat,
    SwapBuffers, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL,
    PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
};
use self::winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExA, DefWindowProcA, DestroyWindow, DispatchMessageA, GetDC,
    GetWindowLongPtrA, MessageBoxA, PeekMessageA, PostMessageA, RegisterClassA, ReleaseDC,
    SetWindowLongPtrA, TranslateMessage, UnregisterClassA, CS_OWNDC, GWLP_USERDATA, MB_ICONERROR,
    MB_OK, MB_TOPMOST, MSG, PM_REMOVE, WM_CREATE, WM_QUIT, WM_SHOWWINDOW, WNDCLASSA, WS_CAPTION,
    WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUPWINDOW, WS_SIZEBOX,
    WS_VISIBLE,
};

use self::winapi::ctypes::c_void;
use crate::Parent::WithParent;
use crate::{handle_message, WindowOpenOptions};
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

fn handle_msg(_window: HWND) -> bool {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if PeekMessageA(&mut msg, 0 as HWND, 0, 0, PM_REMOVE) == 0 {
                return true;
            }
            if msg.message == WM_QUIT {
                return false;
            }
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let win = GetWindowLongPtrA(hwnd, GWLP_USERDATA) as *const c_void;
    match msg {
        WM_CREATE => {
            PostMessageA(hwnd, WM_SHOWWINDOW, 0, 0);
            0
        }
        _ => {
            if !win.is_null() {
                let win: &Arc<Mutex<Window>> = std::mem::transmute(win);
                let win = Arc::clone(win);
                let ret = handle_message(win, msg, wparam, lparam);

                // todo: need_reconfigure thing?

                // return ret
            }
            return DefWindowProcA(hwnd, msg, wparam, lparam);
        }
    }
}

unsafe fn register_wnd_class() -> ATOM {
    // We generate a unique name for the new window class to prevent name collisions
    let class_name = format!("Baseview-{}", generate_guid()).as_ptr() as *const i8;

    let wnd_class = WNDCLASSA {
        style: CS_OWNDC,
        lpfnWndProc: Some(wnd_proc),
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

unsafe fn init_gl_context() {}

pub struct Window {
    hwnd: HWND,
    hdc: HDC,
    gl_context: HGLRC,
    window_class: ATOM,
}

impl Window {
    pub fn open(options: WindowOpenOptions) -> Arc<Mutex<Self>> {
        unsafe {
            let mut window = Window {
                hwnd: null_mut(),
                hdc: null_mut(),
                gl_context: null_mut(),
                window_class: 0,
            };

            let title = (options.title.to_owned() + "\0").as_ptr() as *const i8;

            window.window_class = register_wnd_class();
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

            window.hwnd = CreateWindowExA(
                0,
                window.window_class as _,
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

            window.hdc = GetDC(window.hwnd);

            //init_gl_context(&window, title);

            let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
            pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
            pfd.nVersion = 1;
            pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
            pfd.iPixelType = PFD_TYPE_RGBA;
            pfd.cColorBits = 32;
            // todo: ask wrl why 24 instead of 32?
            pfd.cDepthBits = 24;
            pfd.cStencilBits = 8;
            pfd.iLayerType = PFD_MAIN_PLANE;

            let pf_id: i32 = ChoosePixelFormat(window.hdc, &pfd);
            if pf_id == 0 {
                // todo: use a more useful return like an Option
                // todo: also launch error message boxes
                return Arc::new(Mutex::new(window));
            }

            if SetPixelFormat(window.hdc, pf_id, &pfd) == 0 {
                // todo: use a more useful return like an Option
                // todo: also launch error message boxes
                return Arc::new(Mutex::new(window));
            }

            window.gl_context = wglCreateContext(window.hdc);
            if window.gl_context == 0 as HGLRC {
                // todo: use a more useful return like an Option
                // todo: also launch error message boxes
                return Arc::new(Mutex::new(window));
            }

            if wglMakeCurrent(window.hdc, window.gl_context) == 0 {
                // todo: use a more useful return like an Option
                // todo: also launch error message boxes
                return Arc::new(Mutex::new(window));
            }

            let h = LoadLibraryA("opengl32.dll\0".as_ptr() as *const i8);
            gl::load_with(|symbol| {
                let symbol = CString::new(symbol.as_bytes()).unwrap();
                let symbol = symbol.as_ptr();
                GetProcAddress(h, symbol) as *const _
            });

            let hwnd = window.hwnd;
            let hdc = window.hdc;

            let win = Arc::new(Mutex::new(window));

            SetWindowLongPtrA(hwnd, GWLP_USERDATA, &Arc::clone(&win) as *const _ as _);

            // todo: decide what to do with the message pump
            if parent.is_null() {
                loop {
                    if !handle_msg(null_mut()) {
                        break;
                    }

                    // todo: pass callback rendering function instead
                    gl::ClearColor(0.3, 0.8, 0.3, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                    SwapBuffers(hdc);
                }
            }

            win
        }
    }

    pub fn close(&self) {
        // todo: see https://github.com/wrl/rutabaga/blob/f30ff67e157375cafdbafe5fb549f1790443a3a8/src/platform/win/window.c#L402
        unsafe {
            wglMakeCurrent(null_mut(), null_mut());
            wglDeleteContext(self.gl_context);
            ReleaseDC(self.hwnd, self.hdc);
            DestroyWindow(self.hwnd);
            unregister_wnd_class(self.window_class);
        }
    }

    pub(crate) fn handle_mouse_motion(&self, x: i32, y: i32) {
        println!("{}, {}", x, y);
    }
}
