use crate::{HandlerError, WindowSize};
use std::cell::RefCell;

pub trait HostMainThreadCaller: Send + 'static {
    fn call_main_thread(&mut self);
}

pub trait HostCallbacks: 'static {
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), HandlerError>;
    fn destroyed(&mut self);
}

pub struct Host {
    #[cfg(target_os = "linux")]
    pub(crate) main_thread: Option<Box<dyn HostMainThreadCaller>>,
    pub(crate) callbacks: Option<RefCell<Box<dyn HostCallbacks>>>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    #[inline]
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            main_thread: None,
            callbacks: None,
        }
    }

    #[inline]
    #[allow(unused)]
    pub fn with_main_thread(mut self, main_thread: impl HostMainThreadCaller) -> Self {
        #[cfg(target_os = "linux")]
        {
            self.main_thread = Some(Box::new(main_thread));
        }

        self
    }

    #[inline]
    pub fn with_callbacks(mut self, callbacks: impl HostCallbacks) -> Self {
        self.callbacks = Some(RefCell::new(Box::new(callbacks)));
        self
    }
}

#[allow(unused)]
impl Host {
    pub(crate) fn notify_destroyed(&self) {
        let Some(callbacks) = &self.callbacks else { return };
        let Ok(mut callbacks) = callbacks.try_borrow_mut() else { return };
        callbacks.destroyed();
    }

    pub(crate) fn request_resize(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        let Some(callbacks) = &self.callbacks else { return Ok(()) };
        let Ok(mut callbacks) = callbacks.try_borrow_mut() else { return Ok(()) };
        callbacks.request_resize(new_size)
    }
}
