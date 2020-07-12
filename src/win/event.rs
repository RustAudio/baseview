use std::sync::{Arc, Mutex, MutexGuard};

use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};

use crate::Window;
use winapi::um::wingdi::SwapBuffers;
use winapi::um::winuser::{
    DefWindowProcA, WM_CLOSE, WM_CREATE, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_MOUSEMOVE, WM_PAINT,
    WM_QUIT, WM_RBUTTONUP, WM_SHOWWINDOW, WM_TIMER,
};

const WIN_FRAME_TIMER: usize = 4242;

unsafe fn handle_timer(win: *mut Window, timer_id: usize) {
    match timer_id {
        WIN_FRAME_TIMER => {
            (*win).draw_frame();
        }
        _ => (),
    }
}

pub(crate) unsafe fn handle_message(
    win: *mut Window,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            (*win).handle_mouse_motion(x, y);
            0
        }
        WM_TIMER => {
            handle_timer(win, wparam);
            0
        }
        WM_PAINT => {
            (*win).draw_frame();
            0
        }
        _ => DefWindowProcA((*win).hwnd, message, wparam, lparam),
    }
}
