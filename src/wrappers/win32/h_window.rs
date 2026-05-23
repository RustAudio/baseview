use crate::wrappers::win32::h_wnd::HWnd;
use std::cell::{Cell, OnceCell};
use std::mem::ManuallyDrop;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::rc::Rc;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub trait WindowImpl: 'static {
    fn after_create(&self, window: &HWnd);
    fn handle_message(
        &self, window: &HWnd, message_code: u32, w_param: WPARAM, l_param: LPARAM,
    ) -> Option<LRESULT>;
    fn destroy_started(&self, window: &HWnd);
}

pub enum LifeCycleWindowMessage<'a> {
    Create { create: &'a CREATESTRUCTW },
    Destroy,
    Message(u32, WPARAM, LPARAM),
}

impl<'a> LifeCycleWindowMessage<'a> {
    pub unsafe fn parse(message: u32, w_param: WPARAM, l_param: LPARAM) -> Option<Self> {
        Some(match message {
            WM_CREATE => Self::Create { create: unsafe { &*(l_param as *const CREATESTRUCTW) } },
            WM_DESTROY => Self::Destroy,
            _ => Self::Message(message, w_param, l_param),
        })
    }
}

type Initializer<W> = dyn FnOnce(&HWnd) -> W + 'static;

// TODO: bikeshed
pub(crate) struct WindowUserData<W> {
    initializer: Cell<Option<Box<Initializer<W>>>>,
    inner_impl: OnceCell<W>,
}

impl<W: WindowImpl> WindowUserData<W> {
    pub fn new(initializer: impl FnOnce(&HWnd) -> W + 'static) -> Rc<Self> {
        Rc::new(Self {
            initializer: Cell::new(Some(Box::new(initializer))),
            inner_impl: OnceCell::new(),
        })
    }

    pub unsafe fn from_raw(raw: NonNull<WindowUserData<W>>) -> Rc<Self> {
        let this = ManuallyDrop::new(Rc::from_raw(raw.as_ptr()));
        Rc::clone(&this)
    }

    pub fn initialize(&self, window: &HWnd) -> Result<(), ()> {
        let Some(initializer) = self.initializer.take() else {
            panic!("AdapterContainer is already initialized");
        };

        // TODO: allow initializer to return error
        if self.inner_impl.set(initializer(window)).is_err() {
            // Should not be possible
            panic!("AdapterContainer is already initialized");
        }

        if let Some(inner) = self.inner_impl.get() {
            inner.after_create(window);
        }

        Ok(())
    }

    pub fn destroy_started(&self, window: &HWnd) {
        if let Some(inner) = self.inner_impl.get() {
            inner.destroy_started(window);
        }
    }

    pub fn handle_message(
        &self, window: &HWnd, message_code: u32, w_param: WPARAM, l_param: LPARAM,
    ) -> Option<LRESULT> {
        if let Some(inner) = self.inner_impl.get() {
            inner.handle_message(window, message_code, w_param, l_param)
        } else {
            None
        }
    }
}

pub(crate) unsafe extern "system" fn wnd_proc<W: WindowImpl>(
    window: HWND, message_code: u32, w_param: WPARAM, l_param: LPARAM,
) -> LRESULT {
    let handle_default = || unsafe { DefWindowProcW(window, message_code, w_param, l_param) };
    let window = unsafe { HWnd::from_ref(&window) };

    let message = unsafe { LifeCycleWindowMessage::parse(message_code, w_param, l_param) };

    // Default handling for all other events
    let Some(message) = message else {
        return handle_default();
    };

    match message {
        LifeCycleWindowMessage::Message(message, w_param, l_param) => {
            let Some(inner_ptr) = window.get_userdata_ptr::<WindowUserData<W>>() else {
                // TODO: log error
                return handle_default();
            };

            let inner = unsafe { WindowUserData::from_raw(inner_ptr) };

            let result = catch_unwind(AssertUnwindSafe(|| {
                inner.handle_message(window, message, w_param, l_param)
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
        LifeCycleWindowMessage::Create { create } => {
            let inner_ptr = create.lpCreateParams as *mut WindowUserData<W>;

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
                Ok(Err(())) | Err(_) => {
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
        LifeCycleWindowMessage::Destroy => {
            let Some(state_ptr) = window.get_userdata_ptr::<WindowUserData<W>>() else {
                // TODO: log error
                return handle_default();
            };

            let state = unsafe { Rc::from_raw(state_ptr.as_ptr()) };
            let _ = catch_unwind(AssertUnwindSafe(|| state.destroy_started(window)));
            let _ = window.set_userdata_ptr(core::ptr::null::<W>());
            let _ = catch_unwind(AssertUnwindSafe(|| drop(state)));

            0
        }
    }
}
