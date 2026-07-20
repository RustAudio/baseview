use crate::{HandlerError, WindowSize};

pub trait HostMainThreadCaller: Send + 'static {
    fn call_main_thread(&mut self);
}

pub trait HostCallbacks: 'static {
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), HandlerError>;
    fn destroyed(&mut self);
}

pub struct Host {
    pub(crate) main_thread: Option<Box<dyn HostMainThreadCaller>>,
    pub(crate) callbacks: Option<Box<dyn HostCallbacks>>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    pub fn new() -> Self {
        Self { main_thread: None, callbacks: None }
    }

    pub fn with_main_thread(mut self, main_thread: impl HostMainThreadCaller) -> Self {
        self.main_thread = Some(Box::new(main_thread));
        self
    }

    pub fn with_callbacks(mut self, callbacks: impl HostCallbacks) -> Self {
        self.callbacks = Some(Box::new(callbacks));
        self
    }
}
