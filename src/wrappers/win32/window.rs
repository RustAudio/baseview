mod data;
mod handle;
mod proc;
mod window_class;

use data::WindowData;
pub use handle::HWnd;
pub use proc::wnd_proc;
use std::ptr::null_mut;
use std::rc::Rc;
use window_class::RegisteredClass;
use windows_core::{Error, Result, HSTRING};

use crate::wrappers::win32::h_instance::HInstance;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, WINDOW_STYLE};

pub trait WindowImpl: 'static {
    /// Called during the processing of the WM_CREATE message, but after this type was properly
    /// initialized.
    ///
    /// Note that any messages sent to the window during this function will result in
    /// [`handle_message`] to be called immediately. Implementations must be ready for that.
    ///
    /// If this returns an error, the window creation is canceled.
    fn after_create(&self, window: HWnd) -> Result<()>;
    unsafe fn handle_message(
        &self, window: HWnd, message_code: u32, w_param: WPARAM, l_param: LPARAM,
    ) -> Option<LRESULT>;
    /// Called during the processing of the WM_DESTROY message, but before any other de-initialization
    /// takes place.
    ///
    /// Note that any messages sent to the window during this function will result in
    /// [`handle_message`] to be called immediately. Implementations must be ready for that.
    ///
    /// This function is not fallible. Any errors will be ignored.
    fn before_destroy(&self, window: HWnd);
}

/// Creates a window from the given settings, with a given [`WindowImpl`] type to handle the message,
/// and an initializer function that type.
///
/// The initialization function is called right after the Win32 window was created
pub fn create_window<W: WindowImpl>(
    title: &HSTRING, flags: WINDOW_STYLE, nc_width: i32, nc_height: i32, parent: HWND,
    initializer: impl FnOnce(HWnd) -> W + 'static,
) -> Result<HWND> {
    let instance = HInstance::get();
    let window_class = RegisteredClass::register_new(instance, Some(wnd_proc::<W>))?;

    let data = WindowData::new(initializer, window_class.clone());

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            window_class.as_atom_ptr(),
            title.as_ptr(),
            flags,
            0,
            0,
            nc_width,
            nc_height,
            parent,
            null_mut(),
            instance.as_raw(),
            Rc::into_raw(data).cast(),
        )
    };

    if hwnd.is_null() {
        return Err(Error::from_win32());
    }

    Ok(hwnd)
}
