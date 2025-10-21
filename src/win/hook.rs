use std::{
    collections::HashSet,
    ffi::c_int,
    ptr,
    sync::{LazyLock, Mutex, RwLock},
};

use winapi::{
    shared::{
        minwindef::{LPARAM, WPARAM},
        windef::{HHOOK, HWND, POINT},
    },
    um::{
        libloaderapi::GetModuleHandleW,
        processthreadsapi::GetCurrentThreadId,
        winuser::{
            CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION, MSG, PM_REMOVE,
            WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
            WM_USER,
        },
    },
};

use crate::win::wnd_proc;

static HOOK: Mutex<Option<KeyboardHook>> = Mutex::new(None);

// track all windows opened by this instance of baseview
// we use an RwLock here since the vast majority of uses (event interceptions)
// will only need to read from the HashSet
static OPEN_WINDOWS: LazyLock<RwLock<HashSet<HWNDWrapper>>> = LazyLock::new(|| RwLock::default());

pub(crate) struct KeyboardHookHandle(HWNDWrapper);

struct KeyboardHook(HHOOK);

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct HWNDWrapper(HWND);

// SAFETY: it's a pointer behind a mutex. we'll live
unsafe impl Send for KeyboardHook {}
unsafe impl Sync for KeyboardHook {}

// SAFETY: ditto
unsafe impl Send for HWNDWrapper {}
unsafe impl Sync for HWNDWrapper {}

impl Drop for KeyboardHookHandle {
    fn drop(&mut self) {
        deinit_keyboard_hook(self.0);
    }
}

// initialize keyboard hook
// some DAWs (particularly Ableton) intercept incoming keyboard messages,
// but we're naughty so we intercept them right back
pub(crate) fn init_keyboard_hook(hwnd: HWND) -> KeyboardHookHandle {
    // register hwnd to global window set
    OPEN_WINDOWS.write().unwrap().insert(HWNDWrapper(hwnd));

    let hook = &mut *HOOK.lock().unwrap();

    if hook.is_some() {
        // keyboard hook already exists, just return handle
        KeyboardHookHandle(HWNDWrapper(hwnd))
    } else {
        // keyboard hook doesn't exist (no windows open before this), create it
        let new_hook = KeyboardHook(unsafe {
            SetWindowsHookExW(
                WH_GETMESSAGE,
                Some(keyboard_hook_callback),
                GetModuleHandleW(ptr::null()),
                GetCurrentThreadId(),
            )
        });

        *hook = Some(new_hook);

        KeyboardHookHandle(HWNDWrapper(hwnd))
    }
}

fn deinit_keyboard_hook(hwnd: HWNDWrapper) {
    let windows = &mut *OPEN_WINDOWS.write().unwrap();

    windows.remove(&hwnd);

    if windows.is_empty() {
        if let Ok(mut hook) = HOOK.lock() {
            if let Some(KeyboardHook(hhook)) = &mut *hook {
                unsafe {
                    UnhookWindowsHookEx(*hhook);
                }

                *hook = None;
            }
        }
    }
}

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
// to a window in OPEN_WINDOWS, and intercept it if so
unsafe fn offer_message_to_baseview(msg: *mut MSG) -> bool {
    let msg = &*msg;

    // if this isn't a keyboard message, ignore it
    match msg.message {
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR => {}

        _ => return false,
    }

    // check if this is one of our windows. if so, intercept it
    let Ok(windows) = OPEN_WINDOWS.read() else { return false };

    if windows.contains(&HWNDWrapper(msg.hwnd)) {
        let _ = wnd_proc(msg.hwnd, msg.message, msg.wParam, msg.lParam);

        return true;
    }

    false
}
