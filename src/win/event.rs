use std::sync::{Arc, Mutex};

use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};

use crate::Window;
use winapi::um::winuser::WM_MOUSEMOVE;

pub(crate) unsafe fn handle_message(
    win: Arc<Mutex<Window>>,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            win.lock().unwrap().handle_mouse_motion(x, y);
            0
        }
        _ => 0,
    }
}
