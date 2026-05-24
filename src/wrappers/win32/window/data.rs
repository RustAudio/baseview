use super::*;
use std::cell::{Cell, OnceCell};
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::rc::Rc;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};

type Initializer<W> = dyn FnOnce(HWnd) -> W + 'static;

pub(crate) struct WindowUserData<W> {
    initializer: Cell<Option<Box<Initializer<W>>>>,
    inner_impl: OnceCell<W>,
    window_class: RegisteredClass,
}

impl<W: WindowImpl> WindowUserData<W> {
    pub fn new(initializer: impl FnOnce(HWnd) -> W + 'static, class: RegisteredClass) -> Rc<Self> {
        Rc::new(Self {
            initializer: Cell::new(Some(Box::new(initializer))),
            inner_impl: OnceCell::new(),
            window_class: class,
        })
    }

    pub unsafe fn from_raw(raw: NonNull<WindowUserData<W>>) -> Rc<Self> {
        let this = ManuallyDrop::new(Rc::from_raw(raw.as_ptr()));
        Rc::clone(&this)
    }

    pub fn initialize(&self, window: HWnd) -> Result<(), ()> {
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

    pub fn destroy_started(&self, window: HWnd) {
        if let Some(inner) = self.inner_impl.get() {
            inner.destroy_started(window);
        }
    }

    pub fn handle_message(
        &self, window: HWnd, message_code: u32, w_param: WPARAM, l_param: LPARAM,
    ) -> Option<LRESULT> {
        if let Some(inner) = self.inner_impl.get() {
            inner.handle_message(window, message_code, w_param, l_param)
        } else {
            None
        }
    }
}
