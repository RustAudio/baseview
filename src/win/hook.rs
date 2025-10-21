use std::{
    ffi::c_int,
    ptr,
    sync::{Mutex, Once},
};

use dtor::dtor;
use winapi::{
    shared::{
        minwindef::{LPARAM, WPARAM},
        windef::{HHOOK, POINT},
    },
    um::{
        libloaderapi::GetModuleHandleA,
        processthreadsapi::GetCurrentThreadId,
        winuser::{
            CallNextHookEx, GetClassNameA, GetWindowLongPtrW, SetWindowsHookExA,
            UnhookWindowsHookEx, GWLP_USERDATA, HC_ACTION, MSG, PM_REMOVE, WH_GETMESSAGE, WM_CHAR,
            WM_KEYDOWN, WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_USER,
        },
    },
};

use crate::win::{wnd_proc, WindowState};

static HOOK: Mutex<WinKeyboardHook> = Mutex::new(WinKeyboardHook::new());
static ONCE: Once = Once::new();

// initialize keyboard hook
// some DAWs (particularly Ableton) intercept incoming keyboard messages,
// but we're naughty so we intercept them right back
//
// this is invoked by Window::open() since Rust doesn't have runtime static ctors
pub(crate) fn init_keyboard_hook() {
    ONCE.call_once(|| {
        HOOK.lock().unwrap().hook = unsafe {
            SetWindowsHookExA(
                WH_GETMESSAGE,
                Some(keyboard_hook_callback),
                GetModuleHandleA(ptr::null()),
                GetCurrentThreadId(),
            )
        };
    });
}

#[dtor]
fn deinit_keyboard_hook() {
    let hook = HOOK.lock().unwrap();

    if !hook.hook.is_null() {
        unsafe {
            UnhookWindowsHookEx(hook.hook);
        }
    }
}

struct WinKeyboardHook {
    hook: HHOOK,
}

impl WinKeyboardHook {
    const fn new() -> Self {
        Self { hook: ptr::null_mut() }
    }
}

// SAFETY: it's a pointer behind a mutex. we'll live
unsafe impl Send for WinKeyboardHook {}
unsafe impl Sync for WinKeyboardHook {}

unsafe extern "system" fn keyboard_hook_callback(
    n_code: c_int, wparam: WPARAM, lparam: LPARAM,
) -> isize {
    let msg = lparam as *mut MSG;

    if n_code == HC_ACTION && wparam == PM_REMOVE as usize && offer_message_to_baseview(msg) {
        *msg = MSG {
            hwnd: ptr::null_mut(),
            message: WM_USER,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT { x: 0, y: 0 },
        };

        0
    } else {
        CallNextHookEx(ptr::null_mut(), n_code, wparam, lparam)
    }
}

// check if `msg` is a keyboard message addressed
// to a baseview window, and intercept it if so
unsafe fn offer_message_to_baseview(msg: *mut MSG) -> bool {
    let msg = &*msg;

    // if this isn't a keyboard message, ignore it
    match msg.message {
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR => {}

        _ => return false,
    }

    // check if this is a baseview window (gross)
    let mut classname = [0u8; 9];

    // SAFETY: It's Probably ASCII Lmao
    if GetClassNameA(msg.hwnd, &mut classname as *mut u8 as *mut i8, 9) != 0 {
        if &classname[0..8] == "Baseview".as_bytes() {
            let _ = wnd_proc(
                msg.hwnd,
                msg.message,
                msg.wParam,
                msg.lParam,
            );

            return true;
        }
    }

    false
}
