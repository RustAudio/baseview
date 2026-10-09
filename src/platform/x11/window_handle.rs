use crate::platform::prelude::{Handler, PlatformResult, WindowShared};
use crate::platform::x11::host_handle::HostHandle;
use crate::window::WindowInitializer;
use crate::WindowContext;
use std::rc::Rc;

pub struct WindowHandle {
    shared: Rc<WindowShared>,
    ev_loop: Rc<HostHandle>,
    handler: Handler,
}

impl WindowHandle {
    pub fn create_window(init: WindowInitializer) -> PlatformResult<Self> {
        //let parent_id = init.settings.parent.as_ref().map(|p| p.inner.window_id);
        let event_loop_handle = HostHandle::new(init.host);
        let shared = WindowShared::create(init.settings, Rc::clone(&event_loop_handle))?;
        let handler = init.builder.build(WindowContext::new(Rc::clone(&shared)))?;

        Ok(Self { shared, handler: Handler::new(handler), ev_loop: event_loop_handle })
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        todo!() // Deinit timers & FD watchers
    }
}
