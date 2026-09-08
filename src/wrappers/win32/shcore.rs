use crate::wrappers::win32::{Module, RawLibrary};
use std::ffi::CStr;
use windows_sys::core::HRESULT;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Graphics::Gdi::HMONITOR;
use windows_sys::Win32::UI::HiDpi::*;

type GetProcessDpiAwareness =
    unsafe extern "system" fn(HANDLE, *mut PROCESS_DPI_AWARENESS) -> HRESULT;

type SetProcessDpiAwareness = unsafe extern "system" fn(PROCESS_DPI_AWARENESS) -> HRESULT;

type GetDpiForMonitor =
    unsafe extern "system" fn(HMONITOR, MONITOR_DPI_TYPE, *mut u32, *mut u32) -> HRESULT;

// Checks the above typedefs match the function definitions from windows_sys
const _: () = {
    let _: GetProcessDpiAwareness = GetProcessDpiAwareness;
    let _: SetProcessDpiAwareness = SetProcessDpiAwareness;
    let _: GetDpiForMonitor = GetDpiForMonitor;
};

#[derive(Copy, Clone)]
pub struct ExtendedShCore {
    pub get_process_dpi_awareness: Option<GetProcessDpiAwareness>,
    pub set_process_dpi_awareness: Option<SetProcessDpiAwareness>,
    pub get_dpi_for_monitor: Option<GetDpiForMonitor>,
}

unsafe impl Module for ExtendedShCore {
    const MODULE_NAME: &'static CStr = c"api-ms-win-shcore-scaling-l1-1-1.dll";

    fn load(library: &RawLibrary) -> Self {
        unsafe {
            Self {
                get_process_dpi_awareness: library.get(c"GetProcessDpiAwareness"),
                set_process_dpi_awareness: library.get(c"SetProcessDpiAwareness"),
                get_dpi_for_monitor: library.get(c"GetDpiForMonitor"),
            }
        }
    }
}
