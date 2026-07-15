use super::*;
use std::cell::{Cell, OnceCell};
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::rc::Rc;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};

type Initializer<W> = dyn FnOnce(HWnd) -> W + 'static;

/// Data owned by the Win32 window.
///
/// This is the data behind the `GWLP_USERDATA` pointer.
pub struct WindowData<W> {
    initializer: Cell<Option<Box<Initializer<W>>>>,
    inner_impl: OnceCell<W>,
    // Keep this around to ensure the class is not de-registered while this window is open
    _window_class: RegisteredClass,
}

impl<W: WindowImpl> WindowData<W> {
    pub fn new(initializer: impl FnOnce(HWnd) -> W + 'static, class: RegisteredClass) -> Rc<Self> {
        Rc::new(Self {
            initializer: Cell::new(Some(Box::new(initializer))),
            inner_impl: OnceCell::new(),
            _window_class: class,
        })
    }

    /// Returns an owned pointer from the given raw pointer, without transferring ownership.
    pub unsafe fn from_raw(raw: NonNull<WindowData<W>>) -> Rc<Self> {
        let this = ManuallyDrop::new(Rc::from_raw(raw.as_ptr()));
        Rc::clone(&this)
    }

    pub fn initialize(&self, window: HWnd) -> core::result::Result<(), crate::platform::Error> {
        let Some(initializer) = self.initializer.take() else {
            panic!("WindowData is already initialized");
        };

        if self.inner_impl.set(initializer(window)).is_err() {
            // Should not be possible
            unreachable!("WindowData is already initialized");
        }

        if let Some(inner) = self.inner_impl.get() {
            inner.after_create(window)?;
        }

        Ok(())
    }

    pub fn destroy_started(&self, window: HWnd) {
        if let Some(inner) = self.inner_impl.get() {
            inner.before_destroy(window);
        }
    }

    pub unsafe fn handle_message(
        &self, window: HWnd, message_code: u32, w_param: WPARAM, l_param: LPARAM,
    ) -> Option<LRESULT> {
        if let Some(inner) = self.inner_impl.get() {
            inner.handle_message(window, message_code, w_param, l_param)
        } else {
            None
        }
    }
}
