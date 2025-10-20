use std::{ffi::c_int, ptr, sync::{Mutex, Once}};

use winapi::{shared::{minwindef::{LPARAM, WPARAM}, windef::{HHOOK, POINT}}, um::winuser::{CallNextHookEx, GetClassNameA, GetWindowLongPtrW, SetWindowsHookExA, UnhookWindowsHookEx, GWLP_USERDATA, HC_ACTION, MSG, PM_REMOVE, WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_USER}};

use crate::win::{wnd_proc_inner, WindowState};


static HOOK: Mutex<WinKeyboardHook> = Mutex::new(WinKeyboardHook::new());
static ONCE: Once = Once::new();


pub fn init_keyboard_hook() {
	ONCE.call_once(|| {
		HOOK.lock().unwrap().attach_hook();
	});
}


struct WinKeyboardHook {
	hook: HHOOK,
}

impl WinKeyboardHook {
	const fn new() -> Self {
		Self {
			hook: ptr::null_mut(),
		}
	}

	fn attach_hook(&mut self) {
		self.hook = unsafe { SetWindowsHookExA(
			WH_GETMESSAGE,
			Some(keyboard_hook_callback),
			ptr::null_mut(),
			0
		) };
	}
}

impl Drop for WinKeyboardHook {
	fn drop(&mut self) {
		if !self.hook.is_null() {
			unsafe { UnhookWindowsHookEx(self.hook); }
		}
	}
}

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
			pt: POINT { x: 0, y: 0 }
		};

		0
	} else {
		CallNextHookEx(ptr::null_mut(), n_code, wparam, lparam)
	}
}


fn offer_message_to_baseview(msg: *mut MSG) -> bool {
	if msg.is_null() || !msg.is_aligned() {
		return false
	}

	let msg = unsafe { *(msg as *const MSG) };

	// if this isn't a keyboard message, ignore it
	match msg.message {
		WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR => {},

		_ => return false
	}

	// check if this is a baseview window (gross)
	unsafe {
		let mut classname = [0u8; 9];

		// SAFETY: It's Probably ASCII Lmao
		if GetClassNameA(msg.hwnd, &mut classname as *mut u8 as *mut i8, 9) != 0 {
			if &classname[0..8] == "Baseview".as_bytes() {
				let window_state_ptr = GetWindowLongPtrW(msg.hwnd, GWLP_USERDATA) as *mut WindowState;

				// should we do anything with the return value here?
				let _ = wnd_proc_inner(
					msg.hwnd,
					msg.message,
					msg.wParam,
					msg.lParam,
					&*window_state_ptr
				);

				return true
			}
		}
	} 
	false
}