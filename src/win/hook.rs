use std::{
    collections::HashSet,
    ffi::c_int,
    ptr,
    sync::{LazyLock, RwLock},
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
            WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_USER,
        },
    },
};

use crate::win::wnd_proc;

// track all windows opened by this instance of baseview
// we use an RwLock here since the vast majority of uses (event interceptions)
// will only need to read from the HashSet
static HOOK_STATE: LazyLock<RwLock<KeyboardHookState>> = LazyLock::new(|| RwLock::default());

pub(crate) struct KeyboardHookHandle(HWNDWrapper);

#[derive(Default)]
struct KeyboardHookState {
    hook: Option<HHOOK>,
    open_windows: HashSet<HWNDWrapper>,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct HWNDWrapper(HWND);

// SAFETY: it's a pointer behind an RwLock. we'll live
unsafe impl Send for KeyboardHookState {}
unsafe impl Sync for KeyboardHookState {}

// SAFETY: we never access the underlying HWND ourselves, just use it as a HashSet entry
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
    let state = &mut *HOOK_STATE.write().unwrap();

    // register hwnd to global window set
    state.open_windows.insert(HWNDWrapper(hwnd));

    if state.hook.is_some() {
        // keyboard hook already exists, just return handle
        KeyboardHookHandle(HWNDWrapper(hwnd))
    } else {
        // keyboard hook doesn't exist (no windows open before this), create it
        let new_hook = unsafe {
            SetWindowsHookExW(
                WH_GETMESSAGE,
                Some(keyboard_hook_callback),
                GetModuleHandleW(ptr::null()),
                GetCurrentThreadId(),
            )
        };

        state.hook = Some(new_hook);

        KeyboardHookHandle(HWNDWrapper(hwnd))
    }
}

fn deinit_keyboard_hook(hwnd: HWNDWrapper) {
    let state = &mut *HOOK_STATE.write().unwrap();

    state.open_windows.remove(&hwnd);

    if state.open_windows.is_empty() {
        if let Some(hhook) = state.hook {
            unsafe {
                UnhookWindowsHookEx(hhook);
            }

            state.hook = None;
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

// check if `msg` is a keyboard message addressed to a window
// in KeyboardHookState::open_windows, and intercept it if so
unsafe fn offer_message_to_baseview(msg: *mut MSG) -> bool {
    let msg = &*msg;

    // if this isn't a keyboard message, ignore it
    match msg.message {
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR => {}

        _ => return false,
    }

    // check if this is one of our windows. if so, intercept it
    if HOOK_STATE.read().unwrap().open_windows.contains(&HWNDWrapper(msg.hwnd)) {
        let _ = wnd_proc(msg.hwnd, msg.message, msg.wParam, msg.lParam);

        return true;
    }

    false
}
