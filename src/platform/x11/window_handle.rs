use super::prelude::*;
use crate::platform::ParentWindowHandle;
use crate::utils::SizingStrategy;
use crate::window::WindowInitializer;
use baseview_host::host::TimerId;
use std::cell::RefCell;
use std::os::fd::RawFd;
use std::rc::Rc;

pub struct WindowHandle {
    ev_loop: OptionRcCell<EventLoop>,
    shared: Rc<WindowShared>,
}

impl WindowHandle {
    pub fn create_window(init: WindowInitializer) -> PlatformResult<Self> {
        let ev_loop = EventLoop::create_window(init)?;
        Ok(Self { shared: Rc::clone(ev_loop.shared()), ev_loop: OptionRcCell::new(ev_loop) })
    }

    pub fn resize(&self, new_size: Size) -> PlatformResult<()> {
        self.handle_evloop(|e| e.resize(new_size))
    }

    /// Returns whether the suggested scale factor is now the one in use.
    pub fn suggest_scale_factor(&self, scale: f64) -> PlatformResult<bool> {
        self.handle_evloop(|e| e.suggest_scale_factor(scale))
    }

    pub fn set_parent(&self, parent: ParentWindowHandle) -> PlatformResult<()> {
        self.handle_evloop(|e| e.set_parent(parent))
    }

    pub fn show(&self) -> PlatformResult<()> {
        self.handle_evloop(|e| e.show())
    }

    pub fn hide(&self) -> PlatformResult<()> {
        self.handle_evloop(|e| e.hide())
    }

    pub fn run_until_closed(&self) -> PlatformResult<()> {
        self.handle_evloop(|e| e.run_until_closed())
    }

    pub fn request_poll(&self) -> PlatformResult<()> {
        self.handle_evloop(|e| e.request_poll())
    }

    pub fn host_fd_callback(&self, fd: RawFd) {
        let Some(event_loop) = self.ev_loop.get() else { return };

        if let Err(e) = event_loop.host_fd_callback(fd) {
            error!("Fatal error: {}", e);
            self.ev_loop.delete();
            return;
        }

        if self.shared.close_requested.get() {
            self.ev_loop.delete();
        }
    }

    pub fn host_timer_callback(&self, id: TimerId) {
        let Some(event_loop) = self.ev_loop.get() else { return };
        event_loop.host_timer_callback(id);

        if self.shared.close_requested.get() {
            self.ev_loop.delete();
        }
    }

    pub fn sizing_strategy(&self) -> SizingStrategy {
        self.shared.sizing_state.strategy()
    }

    pub fn size(&self) -> WindowSize {
        self.shared.sizing_state.window_size()
    }

    pub fn waker(&self) -> WindowWaker {
        self.shared.waker()
    }

    pub fn is_open(&self) -> bool {
        self.ev_loop.is_some()
    }
}

impl WindowHandle {
    /// Runs `handler` on the event loop. Once the window is closed this does nothing, and returns
    /// the default value (`()`, or `false`: a closed window has used nothing it was given).
    fn handle_evloop<T: Default>(
        &self, handler: impl FnOnce(&EventLoop) -> PlatformResult<T>,
    ) -> PlatformResult<T> {
        let Some(event_loop) = self.ev_loop.get() else { return Ok(T::default()) };
        let res = handler(&event_loop);

        if self.shared.close_requested.get() {
            self.ev_loop.delete();
        }

        res
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        self.shared.close_requested_from_host.set(true);
    }
}

struct OptionRcCell<T>(RefCell<Option<Rc<T>>>);

impl<T> OptionRcCell<T> {
    pub fn new(value: T) -> Self {
        OptionRcCell(RefCell::new(Some(Rc::new(value))))
    }

    pub fn get(&self) -> Option<Rc<T>> {
        Some(Rc::clone(self.0.borrow().as_ref()?))
    }

    pub fn is_some(&self) -> bool {
        self.0.borrow().is_some()
    }

    pub fn delete(&self) {
        self.0.borrow_mut().take();
    }
}
