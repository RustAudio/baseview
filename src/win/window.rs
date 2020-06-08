extern crate winapi;

use std::ffi::CString;
use std::ptr::null_mut;

use self::winapi::shared::guiddef::GUID;
use self::winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use self::winapi::shared::windef::{HGLRC, HWND};
use self::winapi::um::combaseapi::CoCreateGuid;
use self::winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};
use self::winapi::um::wingdi::{
    wglCreateContext, wglMakeCurrent, ChoosePixelFormat, SetPixelFormat, SwapBuffers,
    PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA,
    PIXELFORMATDESCRIPTOR,
};
use self::winapi::um::winuser::{
    CreateWindowExA, DefWindowProcA, DispatchMessageA, GetDC, MessageBoxA, PeekMessageA,
    PostQuitMessage, RegisterClassA, TranslateMessage, CS_HREDRAW, CS_OWNDC, CS_VREDRAW,
    CW_USEDEFAULT, MB_ICONERROR, MB_OK, MB_TOPMOST, MSG, PM_REMOVE, WM_DESTROY, WM_QUIT, WNDCLASSA,
    WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUPWINDOW,
    WS_SIZEBOX, WS_VISIBLE,
};

use crate::Parent::WithParent;
use crate::WindowOpenOptions;

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

pub struct Window;

impl Window {
    // todo: we should decide this interface
    pub fn open(options: WindowOpenOptions) -> Self {
        unsafe {
            // We generate a unique name for the new window class to prevent name collisions
            let class_name = format!("Baseview-{}", generate_guid());

            let wnd_class = WNDCLASSA {
                // todo: for OpenGL, will use it later
                style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: null_mut(),
                lpszClassName: class_name.as_ptr() as *const i8,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: std::ptr::null::<i8>(),
            };
            RegisterClassA(&wnd_class);

            let mut flags = WS_POPUPWINDOW
                | WS_CAPTION
                | WS_VISIBLE
                | WS_SIZEBOX
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_CLIPSIBLINGS;

            let mut parent = null_mut();
            if let WithParent(p) = options.parent {
                parent = p;
                flags = WS_CHILD | WS_VISIBLE;
            }

            let hwnd = CreateWindowExA(
                0,
                class_name.as_ptr() as *const i8,
                (options.title.to_owned() + "\0").as_ptr() as *const i8,
                // todo: fine for now, will have to change with a parent
                flags,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                // todo: check if usize fits into i32
                options.width as i32,
                options.height as i32,
                parent as *mut _,
                null_mut(),
                null_mut(),
                null_mut(),
            );

            let hdc = GetDC(hwnd);

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

            let pf_id: i32 = ChoosePixelFormat(hdc, &pfd);
            if pf_id == 0 {
                return Window;
            }

            if SetPixelFormat(hdc, pf_id, &pfd) == 0 {
                return Window;
            }

            let gl_context: HGLRC = wglCreateContext(hdc);
            if gl_context == 0 as HGLRC {
                return Window;
            }

            if wglMakeCurrent(hdc, gl_context) == 0 {
                return Window;
            }

            let h = LoadLibraryA("opengl32.dll\0".as_ptr() as *const i8);
            gl::load_with(|symbol| {
                let symbol = CString::new(symbol.as_bytes()).unwrap();
                let symbol = symbol.as_ptr();
                GetProcAddress(h, symbol) as *const _
            });

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
        }

        Window
    }
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
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        _ => {
            return DefWindowProcA(hwnd, msg, w_param, l_param);
        }
    }
    0
}
