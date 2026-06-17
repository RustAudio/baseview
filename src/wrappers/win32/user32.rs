use std::ffi::c_void;
use std::mem::transmute;
use std::ptr::NonNull;
use windows_core::{s, Error, PCSTR};
use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{FreeLibrary, HWND, RECT};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT;
use windows_sys::Win32::UI::WindowsAndMessaging::{WINDOW_EX_STYLE, WINDOW_STYLE};

pub struct ExtendedUser32 {
    _library: LibraryModule,
    pub set_thread_dpi_awareness_context: Option<SetThreadDpiAwarenessContext>,
    pub adjust_window_rect_ex_for_dpi: Option<AdjustWindowRectExForDpi>,
    pub get_dpi_for_window: Option<GetDpiForWindow>,
}

type SetThreadDpiAwarenessContext =
    unsafe extern "system" fn(DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS_CONTEXT;

type GetDpiForWindow = unsafe extern "system" fn(HWND) -> u32;

type AdjustWindowRectExForDpi = unsafe extern "system" fn(
    lprect: *mut RECT,
    dwstyle: WINDOW_STYLE,
    bmenu: BOOL,
    dwexstyle: WINDOW_EX_STYLE,
    dpi: u32,
) -> BOOL;

impl ExtendedUser32 {
    pub fn load() -> Result<Self, Error> {
        let library = unsafe { LibraryModule::load(s!("user32.dll"))? };

        unsafe {
            Ok(Self {
                set_thread_dpi_awareness_context: library
                    .get_proc_address(s!("SetThreadDpiAwarenessContext"))
                    .map(|p| transmute::<*const c_void, SetThreadDpiAwarenessContext>(p)),
                adjust_window_rect_ex_for_dpi: library
                    .get_proc_address(s!("AdjustWindowRectExForDpi"))
                    .map(|p| transmute::<*const c_void, AdjustWindowRectExForDpi>(p)),
                get_dpi_for_window: library
                    .get_proc_address(s!("GetDpiForWindow"))
                    .map(|p| transmute::<*const c_void, GetDpiForWindow>(p)),
                _library: library,
            })
        }
    }
}

impl Clone for ExtendedUser32 {
    fn clone(&self) -> Self {
        let library = unsafe { LibraryModule::load(s!("user32.dll")).unwrap() };
        Self {
            _library: library,

            set_thread_dpi_awareness_context: *self.set_thread_dpi_awareness_context,
            adjust_window_rect_ex_for_dpi: *self.adjust_window_rect_ex_for_dpi,
            get_dpi_for_window: *self.get_dpi_for_window,
        }
    }
}

struct LibraryModule(NonNull<c_void>);

impl LibraryModule {
    pub unsafe fn load(module_name: PCSTR) -> Result<Self, Error> {
        let library = unsafe { LoadLibraryA(module_name.as_ptr()) };
        let Some(library) = NonNull::new(library) else { return Err(Error::from_win32()) };

        Ok(Self(library))
    }

    pub unsafe fn get_proc_address(&self, name: PCSTR) -> Option<*const c_void> {
        let addr = unsafe { GetProcAddress(self.0.as_ptr(), name.as_ptr()) };

        addr.map(|f| f as _)
    }
}

impl Drop for LibraryModule {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0.as_ptr()) };
    }
}
