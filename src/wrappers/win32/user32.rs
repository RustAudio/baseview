use crate::wrappers::win32::{Module, RawLibrary};
use std::ffi::CStr;
use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{HANDLE, HWND, RECT};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{WINDOW_EX_STYLE, WINDOW_STYLE};

type AdjustWindowRectExForDpi =
    unsafe extern "system" fn(*mut RECT, WINDOW_STYLE, BOOL, WINDOW_EX_STYLE, u32) -> BOOL;
pub type AreDpiAwarenessContextsEqual =
    unsafe extern "system" fn(DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT) -> BOOL;
type EnableNonClientDpiScaling = unsafe extern "system" fn(HWND) -> BOOL;
type GetDpiAwarenessContextForProcess = unsafe extern "system" fn(HANDLE) -> DPI_AWARENESS_CONTEXT;
type GetDpiForSystem = unsafe extern "system" fn() -> u32;
type GetDpiForWindow = unsafe extern "system" fn(HWND) -> u32;
type GetDpiFromDpiAwarenessContext = unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> u32;
type GetSystemDpiForProcess = unsafe extern "system" fn(HANDLE) -> u32;
type GetWindowDpiAwarenessContext = unsafe extern "system" fn(HWND) -> DPI_AWARENESS_CONTEXT;
type GetWindowDpiHostingBehavior = unsafe extern "system" fn(HWND) -> DPI_HOSTING_BEHAVIOR;
type IsValidDpiAwarenessContext = unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> BOOL;
type SetProcessDpiAwarenessContext = unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> BOOL;
type SetThreadDpiAwarenessContext =
    unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS_CONTEXT;

// Checks the above typedefs match the function definitions from windows_sys
const _: () = {
    let _: AdjustWindowRectExForDpi = AdjustWindowRectExForDpi;
    let _: AreDpiAwarenessContextsEqual = AreDpiAwarenessContextsEqual;
    let _: EnableNonClientDpiScaling = EnableNonClientDpiScaling;
    let _: GetDpiAwarenessContextForProcess = GetDpiAwarenessContextForProcess;
    let _: GetDpiForSystem = GetDpiForSystem;
    let _: GetDpiForWindow = GetDpiForWindow;
    let _: GetDpiFromDpiAwarenessContext = GetDpiFromDpiAwarenessContext;
    let _: GetSystemDpiForProcess = GetSystemDpiForProcess;
    let _: GetWindowDpiAwarenessContext = GetWindowDpiAwarenessContext;
    let _: GetWindowDpiHostingBehavior = GetWindowDpiHostingBehavior;
    let _: IsValidDpiAwarenessContext = IsValidDpiAwarenessContext;
    let _: SetProcessDpiAwarenessContext = SetProcessDpiAwarenessContext;
    let _: SetThreadDpiAwarenessContext = SetThreadDpiAwarenessContext;
};

#[derive(Copy, Clone)]
pub struct ExtendedUser32 {
    pub adjust_window_rect_ex_for_dpi: Option<AdjustWindowRectExForDpi>,
    pub are_dpi_awareness_contexts_equal: Option<AreDpiAwarenessContextsEqual>,
    pub enable_non_client_dpi_scaling: Option<EnableNonClientDpiScaling>,
    pub get_dpi_awareness_context_for_process: Option<GetDpiAwarenessContextForProcess>,
    pub get_dpi_for_system: Option<GetDpiForSystem>,
    pub get_dpi_for_window: Option<GetDpiForWindow>,
    pub get_dpi_from_dpi_awareness_context: Option<GetDpiFromDpiAwarenessContext>,
    pub get_system_dpi_for_process: Option<GetSystemDpiForProcess>,
    pub get_window_dpi_awareness_context: Option<GetWindowDpiAwarenessContext>,
    pub get_window_dpi_hosting_behavior: Option<GetWindowDpiHostingBehavior>,
    pub is_valid_dpi_awareness_context: Option<IsValidDpiAwarenessContext>,
    pub set_process_dpi_awareness_context: Option<SetProcessDpiAwarenessContext>,
    pub set_thread_dpi_awareness_context: Option<SetThreadDpiAwarenessContext>,
}

unsafe impl Module for ExtendedUser32 {
    const MODULE_NAME: &'static CStr = c"user32.dll";

    fn load(library: &RawLibrary) -> Self {
        unsafe {
            Self {
                adjust_window_rect_ex_for_dpi: library.get(c"AdjustWindowRectExForDpi"),
                are_dpi_awareness_contexts_equal: library.get(c"AreDpiAwarenessContextsEqual"),
                enable_non_client_dpi_scaling: library.get(c"EnableNonClientDpiScaling"),
                get_dpi_awareness_context_for_process: library
                    .get(c"GetDpiAwarenessContextForProcess"),
                get_dpi_for_window: library.get(c"GetDpiForWindow"),
                get_dpi_for_system: library.get(c"GetDpiForSystem"),
                get_dpi_from_dpi_awareness_context: library.get(c"GetDpiFromDpiAwarenessContext"),
                get_system_dpi_for_process: library.get(c"GetSystemDpiForProcess"),
                get_window_dpi_awareness_context: library.get(c"GetWindowDpiAwarenessContext"),
                get_window_dpi_hosting_behavior: library.get(c"GetWindowDpiHostingBehavior"),
                is_valid_dpi_awareness_context: library.get(c"IsValidDpiAwarenessContext"),
                set_process_dpi_awareness_context: library.get(c"SetProcessDpiAwarenessContext"),
                set_thread_dpi_awareness_context: library.get(c"SetThreadDpiAwarenessContext"),
            }
        }
    }
}
