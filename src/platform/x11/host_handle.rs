use baseview_host::dpi::WindowSize;
use baseview_host::host::*;
use std::os::fd::RawFd;
use std::rc::Rc;

pub struct HostHandle {
    callbacks: Option<Box<dyn HostCallbacks>>,
    fd: Option<Box<dyn HostFdSupport>>,
    timer: Option<Box<dyn HostTimerSupport>>,
}

impl HostHandle {
    pub(crate) fn new(mut host: Host) -> Rc<Self> {
        Rc::new(Self {
            callbacks: host.take_callbacks(),
            fd: host.take_fd(),
            timer: host.take_timer(),
        })
    }

    pub fn is_standalone(&self) -> bool {
        self.callbacks.is_none() && self.fd.is_none() && self.timer.is_none()
    }

    pub fn notify_destroyed(&self) {
        if let Some(callbacks) = &self.callbacks {
            callbacks.destroyed()
        }
    }

    pub fn request_resize(&self, new_size: WindowSize) -> Result<(), ()> {
        let Some(callbacks) = &self.callbacks else { return Ok(()) };

        if let Err(e) = callbacks.request_resize(new_size) {
            crate::error!("Host failed to resize parent window: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn timer_support(&self) -> Option<&dyn HostTimerSupport> {
        self.timer.as_deref()
    }

    pub fn fd_support(&self) -> Option<&dyn HostFdSupport> {
        self.fd.as_deref()
    }

    pub fn unregister_timer(&self, id: TimerId) -> Result<(), ()> {
        let Some(callbacks) = &self.timer else { unreachable!() };

        if let Err(e) = callbacks.unregister_timer(id) {
            crate::warn!("Host failed to unregister timer: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn unregister_fd(&self, fd: RawFd) -> Result<(), ()> {
        let Some(callbacks) = &self.fd else { unreachable!() };

        if let Err(e) = callbacks.unregister_fd(fd) {
            crate::warn!("Host failed to unregister fd: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }
}
