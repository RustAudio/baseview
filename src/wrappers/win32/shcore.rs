use crate::wrappers::win32::{Module, RawLibrary};
use std::ffi::CStr;
use windows_sys::core::{BOOL, HRESULT};
use windows_sys::Win32::Foundation::{HANDLE, HWND, RECT};
use windows_sys::Win32::UI::HiDpi::*;

type GetProcessDpiAwareness =
    unsafe extern "system" fn(HANDLE, *mut PROCESS_DPI_AWARENESS) -> HRESULT;

type SetProcessDpiAwareness = unsafe extern "system" fn(PROCESS_DPI_AWARENESS) -> HRESULT;

// Checks the above typedefs match the function definitions from windows_sys
const _: () = {
    let _: GetProcessDpiAwareness = GetProcessDpiAwareness;
    let _: SetProcessDpiAwareness = SetProcessDpiAwareness;
};

#[derive(Copy, Clone)]
pub struct ExtendedShCore {
    pub get_process_dpi_awareness: Option<GetProcessDpiAwareness>,
    pub set_process_dpi_awareness: Option<SetProcessDpiAwareness>,
}

impl ExtendedShCore {
    pub fn can_handle_process_dpi_awareness(&self) -> bool {
        self.get_process_dpi_awareness.is_some() && self.set_process_dpi_awareness.is_some()
    }
}

unsafe impl Module for ExtendedShCore {
    const MODULE_NAME: &'static CStr = c"api-ms-win-shcore-scaling-l1-1-1.dll";

    fn load(library: &RawLibrary) -> Self {
        unsafe {
            Self {
                get_process_dpi_awareness: library.get(c"GetProcessDpiAwareness"),
                set_process_dpi_awareness: library.get(c"SetProcessDpiAwareness"),
            }
        }
    }
}
