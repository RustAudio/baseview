use std::sync::{Arc, Mutex};

use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};

use crate::{AppWindow, Window};
use winapi::um::winuser::{DefWindowProcA, WM_MOUSEMOVE, WM_PAINT, WM_TIMER};

const WIN_FRAME_TIMER: usize = 4242;

unsafe fn handle_timer<A: AppWindow>(win: Arc<Mutex<Window<A>>>, timer_id: usize) {
    match timer_id {
        WIN_FRAME_TIMER => {}
        _ => (),
    }
}

pub(crate) unsafe fn handle_message<A: AppWindow>(
    win: Arc<Mutex<Window<A>>>,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd;
    {
        hwnd = win.lock().unwrap().hwnd;
    }
    match message {
        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            win.lock().unwrap().handle_mouse_motion(x, y);
            0
        }
        WM_TIMER => {
            handle_timer(win, wparam);
            0
        }
        WM_PAINT => 0,
        _ => DefWindowProcA(hwnd, message, wparam, lparam),
    }
}
