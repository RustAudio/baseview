use libloading::{Library, Symbol};
use winapi::shared::minwindef::{BOOL, UINT};
use winapi::shared::windef::{DPI_AWARENESS_CONTEXT, HWND};

/// Provides access to some Win32 API functions that are not available in older Windows versions.
///
/// This is better than eagerly linking to these functions because then the resulting binary
/// wouldn't work *at all* in the older Windows versions, whereas with this approach, we can
/// fall back to alternative logic or alternative values on a case-by-case basis.  
pub struct DynamicWinApi {
    user32_library: Library,
}

impl DynamicWinApi {
    /// Loads the dynamic windows API, in particular "user32.dll".
    pub fn load() -> Self {
        unsafe { Self { user32_library: Library::new("user32.dll").unwrap() } }
    }

    /// Should be available from Windows 10 onwards.
    pub fn set_process_dpi_awareness_context(
        &self,
    ) -> Option<Symbol<SetProcessDpiAwarenessContext>> {
        unsafe { self.user32_library.get(b"SetProcessDpiAwarenessContext").ok() }
    }

    /// Should be available from Windows 10 onwards.
    pub fn get_dpi_for_window(&self) -> Option<Symbol<GetDpiForWindow>> {
        unsafe { self.user32_library.get(b"GetDpiForWindow").ok() }
    }
}

type SetProcessDpiAwarenessContext = extern "stdcall" fn(value: DPI_AWARENESS_CONTEXT) -> BOOL;

type GetDpiForWindow = extern "stdcall" fn(hwnd: HWND) -> UINT;
