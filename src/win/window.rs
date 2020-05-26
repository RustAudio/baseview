extern crate winapi;

use self::winapi::_core::mem::MaybeUninit;
use self::winapi::shared::guiddef::GUID;
use self::winapi::shared::minwindef::{LPARAM, LPVOID, LRESULT, UINT, WPARAM};
use self::winapi::shared::windef::{HBRUSH, HICON, HMENU, HWND};
use self::winapi::um::combaseapi::CoCreateGuid;
use self::winapi::um::libloaderapi::GetModuleHandleA;
use self::winapi::um::winuser::{
    CreateWindowExA, DefWindowProcA, DispatchMessageA, PeekMessageA, PostQuitMessage,
    RegisterClassA, TranslateMessage, CS_HREDRAW, CS_OWNDC, CS_VREDRAW, CW_USEDEFAULT, MSG,
    PM_REMOVE, WM_DESTROY, WM_QUIT, WNDCLASSA, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

use crate::WindowOpenOptions;

pub fn handle_msg(_window: HWND) -> bool {
    unsafe {
        let mut msg: MSG = MaybeUninit::uninit().assume_init();
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

pub unsafe extern "system" fn wnd_proc(
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
    return 0;
}

pub fn create_window(w: WindowOpenOptions) {
    unsafe {
        // We generate a unique name for the new window class to prevent name collisions
        let mut guid: GUID = MaybeUninit::uninit().assume_init();
        CoCreateGuid(&mut guid);
        let class_name = format!(
            "Baseview-{:0X}-{:0X}-{:0X}-{:0X}{:0X}-{:0X}{:0X}{:0X}{:0X}{:0X}{:0X}\0",
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
        );

        let hinstance = GetModuleHandleA(0 as *const i8);
        let wnd_class = WNDCLASSA {
            // todo: for OpenGL, will use it later
            style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name.as_ptr() as *const i8,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: 0 as HICON,
            hCursor: 0 as HICON,
            hbrBackground: 0 as HBRUSH,
            lpszMenuName: 0 as *const i8,
        };
        RegisterClassA(&wnd_class);

        let _hwnd = CreateWindowExA(
            0,
            class_name.as_ptr() as *const i8,
            (w.title.to_owned() + "\0").as_ptr() as *const i8,
            // todo: fine for now, will have to change with a parent
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            // todo: check if usize fits into i32
            w.width as i32,
            w.height as i32,
            0 as HWND,
            0 as HMENU,
            hinstance,
            0 as LPVOID,
        );
    }
}
