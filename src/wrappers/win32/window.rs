mod data;
mod handle;
mod proc;
mod window_class;

use data::WindowData;
use dpi::PhysicalSize;
pub use handle::HWnd;
pub use proc::wnd_proc;
use std::ptr::null_mut;
use std::rc::Rc;
use window_class::RegisteredClass;
use windows_core::{Error, Result, HSTRING};

use crate::wrappers::win32::h_instance::HInstance;
use crate::wrappers::win32::style::WindowStyle;
use crate::wrappers::win32::DpiAwarenessContext;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW;

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
/// and an initializer function for that type.
///
/// The initialization function is called during the handling of the WM_CREATE function, which allows
/// it to receive a valid HWND, but before it has to handle any messages.
///
/// Note that any message sent to the window by the given `initializer` will not be sent to the
/// [`WindowImpl::handle_message`] function.
/// For any non-trivial operations (e.g. window resizing, GL context creation, etc.), put them in
/// [`WindowImpl::after_create`] instead.
pub fn create_window<W: WindowImpl>(
    title: &HSTRING, style: WindowStyle, nc_size: PhysicalSize<u32>, parent: HWND,
    _dpi_ctx: &DpiAwarenessContext, initializer: impl FnOnce(HWnd) -> W + 'static,
) -> Result<HWND> {
    let instance = HInstance::get();
    let window_class = RegisteredClass::register_new(instance, Some(wnd_proc::<W>))?;

    let data = WindowData::new(initializer, window_class.clone());

    let hwnd = unsafe {
        CreateWindowExW(
            style.style_ex,
            window_class.as_atom_ptr(),
            title.as_ptr(),
            style.style,
            0,
            0,
            nc_size.width.try_into().unwrap_or(i32::MAX),
            nc_size.height.try_into().unwrap_or(i32::MAX),
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
