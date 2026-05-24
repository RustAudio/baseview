use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::rc::Rc;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub unsafe extern "system" fn wnd_proc<W: WindowImpl>(
    window: HWND, message_code: u32, w_param: WPARAM, l_param: LPARAM,
) -> LRESULT {
    let handle_default = || unsafe { DefWindowProcW(window, message_code, w_param, l_param) };
    let window = unsafe { HWnd::from_raw(window) };

    match message_code {
        WM_CREATE => {
            let create = unsafe { &*(l_param as *const CREATESTRUCTW) };
            let inner_ptr = create.lpCreateParams as *mut WindowData<W>;

            let Some(inner_ptr) = NonNull::new(inner_ptr) else {
                // If the state pointer was null for some weird reason, we just abort.
                // TODO: log error
                return -1;
            };

            if let Err(_e) = window.set_userdata_ptr(inner_ptr.as_ptr()) {
                // The call to SetWindowLongPtrW failed for some reason, we cannot continue.

                // Try to recover and free the received pointer data. But if this also fails, better to leak
                // it than risk crashing
                let _ = catch_unwind(AssertUnwindSafe(|| drop(Rc::from_raw(inner_ptr.as_ptr()))));

                // TODO: log error
                return -1;
            }

            // Now the fun begins
            let result = catch_unwind(AssertUnwindSafe(|| {
                let inner = unsafe { inner_ptr.as_ref() };

                inner.initialize(window)
            }));

            match result {
                // If successful, all good.
                // Ownership of the inner state has been passed to the window via the userdata ptr.
                Ok(Ok(())) => 0,

                // If initializer failed or errored, abort.
                Ok(Err(_)) | Err(_) => {
                    // First, revoke ownership from the window, we don't want it to be used by any subsequent messages.
                    let _ = window.set_userdata_ptr(core::ptr::null::<W>());

                    // Try to recover and free the received pointer data. But if this also fails, better to leak
                    // it than risk crashing
                    let _ =
                        catch_unwind(AssertUnwindSafe(|| drop(Rc::from_raw(inner_ptr.as_ptr()))));

                    // TODO: log error
                    -1
                }
            }
        }
        WM_DESTROY => {
            let Some(state_ptr) = window.get_userdata_ptr::<WindowData<W>>() else {
                // TODO: log error
                return handle_default();
            };

            let state = unsafe { Rc::from_raw(state_ptr.as_ptr()) };
            let _ = catch_unwind(AssertUnwindSafe(|| state.destroy_started(window)));
            let _ = window.set_userdata_ptr(core::ptr::null::<W>());
            let _ = catch_unwind(AssertUnwindSafe(|| drop(state)));

            0
        }
        _ => {
            let Some(inner_ptr) = window.get_userdata_ptr::<WindowData<W>>() else {
                // TODO: log error
                return handle_default();
            };

            let inner = unsafe { WindowData::from_raw(inner_ptr) };

            let result = catch_unwind(AssertUnwindSafe(|| {
                inner.handle_message(window, message_code, w_param, l_param)
            }));

            let _ = catch_unwind(AssertUnwindSafe(|| drop(inner)));

            match result {
                Ok(result) => result.unwrap_or_else(handle_default),
                Err(_) => {
                    // TODO: log error
                    unsafe { DestroyWindow(window.as_raw()) }; // TODO: check error
                    -1
                }
            }
        }
    }
}
