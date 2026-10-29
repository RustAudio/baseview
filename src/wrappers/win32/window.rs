mod data;
mod handle;
mod interfaces;
mod proc;
mod window_class;

pub use interfaces::*;

#[cfg(feature = "opengl")]
mod dc;

#[cfg(feature = "opengl")]
pub use dc::*;
use std::mem::ManuallyDrop;
#[cfg(feature = "opengl")]
mod wgl;
#[cfg(feature = "opengl")]
pub use wgl::*;

use crate::dpi::PhysicalSize;
pub use data::WindowData;
pub use handle::*;
pub use proc::wnd_proc;
use std::ptr::{null_mut, NonNull};
use std::rc::Rc;
use window_class::RegisteredClass;
use windows_core::{Error, Result, HSTRING};

use crate::wrappers::win32::dpi::DpiAwarenessGuard;
use crate::wrappers::win32::h_instance::HInstance;
use crate::wrappers::win32::style::WindowStyle;
use windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW;

pub trait WindowImpl: WmSizing + WmMouse + WmKeyboard + WmDraw + WmCursor + 'static {
    fn non_client_create(
        &self, window: HWnd,
    ) -> core::result::Result<(), crate::platform::PlatformError>;

    /// Called during the processing of the WM_CREATE message, but after this type was properly
    /// initialized.
    ///
    /// Note that any messages sent to the window during this function will result in
    /// [`handle_message`] to be called immediately. Implementations must be ready for that.
    ///
    /// If this returns an error, the window creation is canceled.
    fn after_create(
        &self, window: HWnd,
    ) -> core::result::Result<(), crate::platform::PlatformError>;

    /// Called during the processing of the WM_DESTROY message, but before any other de-initialization
    /// takes place.
    ///
    /// Note that any messages sent to the window during this function will result in
    /// [`handle_message`] to be called immediately. Implementations must be ready for that.
    ///
    /// This function is not fallible. Any errors will be ignored.
    fn before_destroy(&self, window: HWnd);

    fn on_close(&self);
    fn on_close_requested(&self, window: HWnd);
    fn on_show_window(&self);
}

/// # Safety
///
/// Must only be called on a window created with create_window, and W must match the inner window type.
pub unsafe fn use_window<W: WindowImpl>(hwnd: HWnd, handler: impl FnOnce(&W)) {
    let Some(raw_data) = hwnd.get_userdata_ptr::<WindowData<W>>() else { return };
    // This is actually borrowed from the window itself
    let raw = ManuallyDrop::new(Rc::from_raw(raw_data.as_ptr()));

    // We keep this alive for as long as inner() runs, to guard against possible re-entrancy
    let window_data = Rc::clone(&raw);
    if let Some(inner) = window_data.inner() {
        handler(inner);
    }
}

/// Creates a window from the given settings, with a given [`WindowImpl`] type to handle the message,
/// and an initializer function for that type.
///
/// The initialization function is called during the handling of the WM_CREATE function, which allows
/// it to receive a valid HWND, but before it has to handle any messages.
///
/// Note that any message sent to the window by the given `initializer` will not be sent to any of
/// its handler methods.
/// For any non-trivial operations (e.g. window resizing, GL context creation, etc.), put them in
/// [`WindowImpl::after_create`] instead.
pub fn create_window<W: WindowImpl>(
    title: &HSTRING, style: WindowStyle, nc_size: PhysicalSize<u32>, parent: Option<HWnd>,
    _dpi_ctx: &DpiAwarenessGuard, initializer: impl FnOnce(HWnd) -> W + 'static,
) -> Result<HWnd> {
    let instance = HInstance::get_from_dll();
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
            parent.map(|p| p.as_raw()).unwrap_or(null_mut()),
            null_mut(),
            instance.as_raw(),
            Rc::into_raw(Rc::clone(&data)).cast(),
        )
    };

    let Some(hwnd) = NonNull::new(hwnd) else { return Err(Error::from_thread()) };
    // SAFETY: This Hwnd is valid since it came from CreateWindowExW
    let hwnd = unsafe { HWnd::from_raw(hwnd) };

    Ok(hwnd)
}

#[cfg(feature = "opengl")]
pub fn with_dummy_window<T>(handler: impl FnOnce(HWnd) -> Result<T>) -> Result<T> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{DefWindowProcW, CW_USEDEFAULT};

    let instance = HInstance::get_from_dll();
    let window_class = RegisteredClass::register_new(instance, Some(DefWindowProcW))?;
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            window_class.as_atom_ptr(),
            null_mut(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            instance.as_raw(),
            null_mut(),
        )
    };

    let Some(hwnd) = NonNull::new(hwnd) else { return Err(Error::from_thread()) };
    // SAFETY: This Hwnd is valid since it came from CreateWindowExW
    let hwnd = unsafe { HWnd::from_raw(hwnd) };

    handler(hwnd)
}
