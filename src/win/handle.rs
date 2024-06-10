use crate::win::win32_window::Win32Window;
use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use std::cell::Cell;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::rc::Rc;
use winapi::shared::windef::HWND;
use winapi::um::winuser::{DispatchMessageW, GetMessageW, TranslateMessage};

struct HandleShared {
    is_open: Cell<bool>,
}

pub struct WindowHandleTransmitter {
    shared: Rc<HandleShared>,
}

impl WindowHandleTransmitter {
    pub unsafe fn new(handle: HWND) -> (WindowHandleTransmitter, WindowHandle) {
        let shared = Rc::new(HandleShared { is_open: Cell::new(true) });

        (
            WindowHandleTransmitter { shared: shared.clone() },
            WindowHandle { shared, inner: Some(handle) },
        )
    }

    pub fn notify_closed(&self) {
        self.shared.is_open.set(false);
    }
}

impl Drop for WindowHandleTransmitter {
    // Note: this is never guaranteed to be called.
    fn drop(&mut self) {
        self.notify_closed()
    }
}

pub struct WindowHandle {
    inner: Option<HWND>,
    shared: Rc<HandleShared>,
}

impl WindowHandle {
    pub(crate) fn block_on_window(mut self) {
        // SAFETY: we keep the handle valid
        unsafe { block_on_running_window(self.inner.take().unwrap()) }
    }

    pub fn close(&mut self) {
        if !self.is_open() {
            return;
        }

        if let Some(hwnd) = self.inner.take() {
            unsafe {
                Win32Window::request_close(hwnd);
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.shared.is_open.get()
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::empty();
        // TODO: add hinstance

        if self.is_open() {
            if let Some(hwnd) = self.inner {
                handle.hwnd = hwnd as *mut c_void;
            }
        }

        handle.into()
    }
}

/// # Safety
/// The handle must be valid.
unsafe fn block_on_running_window(hwnd: HWND) {
    let mut msg = MaybeUninit::zeroed();

    loop {
        let status = unsafe { GetMessageW(msg.as_mut_ptr(), hwnd, 0, 0) };

        if status == -1 {
            break;
        }

        let msg = msg.assume_init_ref();

        unsafe {
            TranslateMessage(msg);
            DispatchMessageW(msg);
        }
    }
}
