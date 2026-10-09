use baseview_host::dpi::WindowSize;
use baseview_host::host::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct HostHandle {
    callbacks: RefCell<HostCallbacksInner>,
}

impl HostHandle {
    pub(crate) fn new(mut host: Host) -> Rc<Self> {
        Rc::new(Self {
            callbacks: RefCell::new(HostCallbacksInner {
                callbacks: host.take_callbacks(),
                fd: host.take_fd(),
                timer: host.take_timer(),
            }),
        })
    }

    pub fn notify_destroyed(&self) {
        let mut callbacks = self.callbacks.borrow_mut();
        if let Some(callbacks) = &mut callbacks.callbacks {
            callbacks.destroyed()
        }
    }

    pub fn request_resize(&self, new_size: WindowSize) -> Result<(), ()> {
        let mut callbacks = self.callbacks.borrow_mut();
        let Some(callbacks) = &mut callbacks.callbacks else { return Ok(()) };

        if let Err(e) = callbacks.request_resize(new_size) {
            crate::error!("Host failed to resize parent window: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }
}

struct HostCallbacksInner {
    callbacks: Option<Box<dyn HostCallbacks>>,
    fd: Option<Box<dyn HostFdSupport>>,
    timer: Option<Box<dyn HostTimerSupport>>,
}
