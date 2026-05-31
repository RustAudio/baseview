pub mod cursor;
pub mod h_instance;
mod rect;
pub mod uuid;
pub mod window;

pub use rect::Rect;

use std::ptr::null_mut;
use windows_core::{Error, Result, HRESULT};
use windows_sys::Win32::Foundation::{S_FALSE, S_OK};
use windows_sys::Win32::System::Ole::OleInitialize;
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

pub fn ole_initialize() -> Result<()> {
    // SAFETY: this is always safe to call with NULL
    match unsafe { OleInitialize(null_mut()) } {
        S_OK | S_FALSE => Ok(()),
        result => Err(Error::new(HRESULT(result), "OLE initialization failed")),
    }
}

pub fn run_thread_message_loop_until(until: impl Fn() -> bool) -> Result<()> {
    let mut msg = MSG::default();

    loop {
        // SAFETY: The msg pointer is valid as it comes from a reference. NULL/0 are valid arguments for this function.
        let result = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };

        match result {
            -1 => return Err(Error::from_win32()), // -1 means error
            0 => return Ok(()),                    // 0 means WM_QUIT was received
            _ => {}                                // Nonzero means a message was retrieved
        }

        // SAFETY: The msg pointer is valid since it comes from a reference.
        // The contents of the msg struct itself are valid, since they come from GetMessage, and we
        // checked the error cases above.
        let _ = unsafe { TranslateMessage(&msg) }; // TODO: log warning if this failed

        // SAFETY: same as above
        unsafe { DispatchMessageW(&msg) };

        if until() {
            return Ok(());
        }
    }
}

// Only works on Windows 10 unfortunately.
pub fn set_process_per_monitor_dpi_aware() {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE) };
}
