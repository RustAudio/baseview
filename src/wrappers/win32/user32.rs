use crate::wrappers::win32::{LibraryModule, Module, RawLibrary};
use std::ffi::{c_void, CStr};
use std::mem::transmute;
use windows_core::{s, Error};
use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{WINDOW_EX_STYLE, WINDOW_STYLE};

pub struct ExtendedUser32 {
    _library: LibraryModule,
    pub set_thread_dpi_awareness_context: Option<SetThreadDpiAwarenessContext>,
    pub adjust_window_rect_ex_for_dpi: Option<AdjustWindowRectExForDpi>,
    pub get_dpi_for_window: Option<GetDpiForWindow>,
    pub set_process_dpi_awareness_context: Option<SetProcessDpiAwarenessContext>,
}

type SetThreadDpiAwarenessContext =
    unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS_CONTEXT;
type SetProcessDpiAwarenessContext = unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> BOOL;

type GetDpiForWindow = unsafe extern "system" fn(HWND) -> u32;

type AdjustWindowRectExForDpi = unsafe extern "system" fn(
    lprect: *mut RECT,
    dwstyle: WINDOW_STYLE,
    bmenu: BOOL,
    dwexstyle: WINDOW_EX_STYLE,
    dpi: u32,
) -> BOOL;

type IsValidDpiAwarenessContext = unsafe extern "system" fn(value: DPI_AWARENESS_CONTEXT) -> BOOL;

// Checks the above typedefs match the function definitions from windows_sys
const _: () = {
    let _: GetDpiForWindow = GetDpiForWindow;
    let _: AdjustWindowRectExForDpi = AdjustWindowRectExForDpi;
    let _: SetThreadDpiAwarenessContext = SetThreadDpiAwarenessContext;
    let _: IsValidDpiAwarenessContext = IsValidDpiAwarenessContext;
};

#[derive(Copy, Clone)]
pub struct ExtendedUser32 {
    pub set_thread_dpi_awareness_context: Option<SetThreadDpiAwarenessContext>,
    pub adjust_window_rect_ex_for_dpi: Option<AdjustWindowRectExForDpi>,
    pub get_dpi_for_window: Option<GetDpiForWindow>,
}

impl Module for ExtendedUser32 {
    const MODULE_NAME: &'static CStr = c"user32.dll";

    fn load(library: &RawLibrary) -> Self {
        unsafe {
            Self {
                set_thread_dpi_awareness_context: library.get(c"SetThreadDpiAwarenessContext"),
                adjust_window_rect_ex_for_dpi: library.get(c"AdjustWindowRectExForDpi"),
                get_dpi_for_window: library.get(c"GetDpiForWindow"),
            }
        }
    }
}
