use super::drag_n_drop::DragNDropState;
use super::keyboard::{convert_key_press_event, convert_key_release_event};
use super::prelude::*;
use crate::platform::x11::error::FatalError;
use crate::platform::x11::handler::Handler;
use crate::platform::x11::host_handle::HostHandle;
use crate::platform::x11::present::PresentState;
use crate::platform::x11::sizing::SizingState;
use crate::platform::ParentWindowHandle;
use crate::utils::SizingStrategy;
use crate::window::WindowInitializer;
use crate::wrappers::poller::{ConnectionPoller, PollStatus};
use crate::wrappers::xkbcommon::XkbcommonState;
use crate::{warn, WindowContext};
use crate::{Event, WindowEvent};
use baseview_host::host::TimerId;
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::result::Result;
use std::time::Instant;
use x11rb::connection::Connection;
use x11rb::protocol::Event as XEvent;

pub(crate) struct EventLoop {
    handler: Handler,
    shared: Rc<WindowShared>,

    drag_n_drop: DragNDropState,
    sizing_state: SizingState,
    pub present_state: PresentState,
    xkb_state: Option<XkbcommonState>,

    host: Rc<HostHandle>,

    run_error: Option<PlatformError>,
    fd_registered: Cell<bool>,
}

impl EventLoop {
    pub fn waker(&self) -> WindowWaker {
        self.shared.waker_source.create_waker()
    }

    pub fn request_poll(&self) -> PlatformResult<()> {
        self.handler.poll();
        Ok(())
    }

    pub fn sizing_strategy(&self) -> SizingStrategy {
        self.shared.sizing_state.strategy()
    }

    pub fn handle_main_thread_callback(&self) {
        todo!()
    }

    pub fn host_timer_callback(&self, timer_id: TimerId) {
        if let Some(timer) = self.shared.timer_manager.get_handle_if_exists(timer_id) {
            self.handler.on_timer(&timer.into());
            return;
        }

        if !self.shared.present_state.trigger_if_redraw_timer(timer_id, &self.host) {
            self.present_state.trigger_possible_fallback_frame_timer(timer_id);
        }
    }

    pub fn host_fd_callback(&self, fd: RawFd) {
        if fd != self.shared.connection.conn.as_raw_fd() {
            return;
        };

        self.flush_drain_xcb_events().unwrap(); // TODO: unwraps
        self.handle_idle().unwrap();
    }

    pub fn max_size(&self) -> Option<Size> {
        self.sizing_strategy().max_size()
    }

    pub fn min_size(&self) -> Option<Size> {
        self.sizing_strategy().min_size()
    }

    pub fn is_resizable(&self) -> bool {
        self.sizing_strategy().is_resizable()
    }

    pub fn is_open(&self) -> bool {
        todo!()
    }

    pub fn run_until_closed(&self) -> PlatformResult<()> {
        self.show()?;

        let connection = Rc::clone(&self.shared.connection);
        let waker_fd = self.shared.waker_source.inner_fd();
        let mut poller = ConnectionPoller::new(&connection.conn, &waker_fd)?;

        self.shared.stop_own_event_loop.set(false);

        while !self.shared.stop_own_event_loop.get() {
            let next_deadline = self.tick_all_timers();
            self.handle_idle()?;

            match poller.wait(next_deadline)? {
                PollStatus::ReadAvailable => {
                    self.flush_drain_xcb_events()?;
                }
                PollStatus::WakerTriggered => {
                    self.shared.waker_source.poll_requests(&self.shared.present_state, &self.host);
                }
                _ => {}
            }

            /*
            // Check if the user has requested the window to close
            if self.window.close_requested.get() {
                self.handle_must_close();
                self.window.close_requested.set(false);
            }*/
        }

        poller.delete()?;
        self.hide()?;

        Ok(())
    }

    pub fn size(&self) -> WindowSize {
        self.shared.sizing_state.window_size()
    }
}

impl EventLoop {
    pub fn create_window(init: WindowInitializer) -> PlatformResult<Self> {
        let parent_id = init.settings.parent.as_ref().map(|p| p.inner.window_id);
        let host = HostHandle::new(init.host);
        // TODO: check deinit runs on initialization error
        let shared = WindowShared::create(init.settings, Rc::clone(&host))?;
        let handler = init.builder.build(WindowContext::new(Rc::clone(&shared)))?;

        let this = Self {
            handler: Handler::new(handler),

            present_state: PresentState::new(&host),
            sizing_state: SizingState::new(parent_id),
            drag_n_drop: DragNDropState::new(),
            xkb_state: XkbcommonState::new(&shared.connection),
            run_error: None,
            fd_registered: false.into(),

            shared,
            host,
        };

        this.register_fd()?;

        Ok(this)
    }

    fn register_fd(&self) -> PlatformResult<()> {
        if let Some(fd_support) = self.host.fd_support() {
            fd_support.register_fd(self.shared.connection.conn.as_fd())?;
            self.fd_registered.set(true);
        } else {
            // TODO
        }

        Ok(())
    }

    fn tick_all_timers(&self) -> Option<Instant> {
        let (deadline_1, triggered_timer) = self.shared.timer_manager.tick_next_timer();

        if let Some(triggered_timer) = triggered_timer {
            self.handler.on_timer(&triggered_timer.into());
        }

        let deadline_2 = self.shared.present_state.tick_timers(&self.host);
        let deadline_3 = self.present_state.tick_fallback_frame_timer();

        fn min_deadline(d1: Option<Instant>, d2: Option<Instant>) -> Option<Instant> {
            match (d1, d2) {
                (Some(d1), Some(d2)) => Some(d2.min(d1)),
                (None, Some(d)) | (Some(d), None) => Some(d),
                (None, None) => None,
            }
        }

        min_deadline(min_deadline(deadline_1, deadline_2), deadline_3)
    }

    #[inline]
    fn flush_drain_xcb_events(&self) -> Result<bool, FatalError> {
        self.shared.connection.conn.flush()?;

        let mut event_received = false;
        while let Some(event) = self.shared.connection.conn.poll_for_event()? {
            event_received = true;
            self.handle_xcb_event(event)?;
        }

        Ok(event_received)
    }

    pub fn stop_now(&self) {
        todo!();
        //self.loop_signal.stop();
        //self.loop_signal.wakeup();
    }

    pub fn trigger_fatal_error(&self, error: PlatformError) {
        if self.run_error.is_none() {
            //self.run_error = Some(error); TODO
        }
        self.stop_now();
    }

    pub fn resize(&self, new_size: Size) -> PlatformResult<()> {
        self.sizing_state.handle_host_resize(new_size, &self.handler, &self.shared)
    }

    pub fn suggest_scale_factor(&self, scale: f64) -> PlatformResult<()> {
        self.sizing_state.handle_host_suggest_scale_factor(scale, &self.handler, &self.shared)
    }

    pub fn set_parent(&self, new_parent: ParentWindowHandle) -> PlatformResult<()> {
        self.shared.xcb_window.reparent(Some(new_parent.window_id))?.check()?;
        Ok(())
    }

    pub fn show(&self) -> PlatformResult<()> {
        self.shared.xcb_window.map_window()?.check()?;
        Ok(())
    }

    pub fn hide(&self) -> PlatformResult<()> {
        self.shared.xcb_window.unmap_window()?.check()?;
        Ok(())
    }

    fn handle_idle(&self) -> Result<(), FatalError> {
        // Check for any events in the internal buffers before going to sleep:
        self.flush_drain_xcb_events()?;

        loop {
            self.sizing_state.handle_coalesced_resize_events(
                &self.shared,
                &self.handler,
                &self.host,
            )?;

            self.present_state.handle_requests(&self.shared, &self.handler, &self.host)?;

            if !self.flush_drain_xcb_events()? {
                break;
            }
        }

        Ok(())
    }

    pub fn shared(&self) -> &WindowShared {
        &self.shared
    }

    fn handle_xcb_event(&self, event: XEvent) -> Result<(), FatalError> {
        match event {
            XEvent::ClientMessage(event) if event.window == self.shared.raw_id() => {
                if event.format != 32 {
                    return Ok(());
                }

                if event.data.as_data32()[0] == self.shared.connection.atoms.WM_DELETE_WINDOW {
                    self.shared.request_close();
                    return Ok(());
                }

                if event.type_ == self.shared.connection.atoms.XdndEnter {
                    self.drag_n_drop.handle_enter_event(&self.shared, &self.handler, &event)?;
                } else if event.type_ == self.shared.connection.atoms.XdndPosition {
                    self.drag_n_drop.handle_position_event(&self.shared, &self.handler, &event)?;
                } else if event.type_ == self.shared.connection.atoms.XdndDrop {
                    self.drag_n_drop.handle_drop_event(&self.shared, &self.handler, &event)?;
                } else if event.type_ == self.shared.connection.atoms.XdndLeave {
                    self.drag_n_drop.handle_leave_event(&self.handler, &event);
                }
            }

            XEvent::SelectionNotify(event) => {
                if event.property == self.shared.connection.atoms.XdndSelection {
                    self.drag_n_drop.handle_selection_notify_event(
                        &self.shared,
                        &self.handler,
                        &event,
                    )?;
                }
            }

            XEvent::Error(e) => {
                warn!("Received leftover X11 error: {:?}", e);
            }

            XEvent::ConfigureNotify(event) => {
                self.sizing_state.handle_configure_notify_event(event, &self.shared.xcb_window);
            }

            ////
            // mouse
            ////
            XEvent::MotionNotify(event) if event.event == self.shared.raw_id() => {
                handle_motion_notify(event, &self.handler)
            }
            XEvent::EnterNotify(event) if event.event == self.shared.raw_id() => {
                handle_enter_notify(event, &self.handler)
            }
            XEvent::LeaveNotify(event) if event.event == self.shared.raw_id() => {
                handle_leave_notify(event, &self.handler)
            }
            XEvent::ButtonPress(event) if event.event == self.shared.raw_id() => {
                handle_button_press(event, &self.handler)
            }
            XEvent::ButtonRelease(event) if event.event == self.shared.raw_id() => {
                handle_button_release(event, &self.handler);
            }

            ////
            // keys
            ////
            XEvent::KeyPress(event) if event.event == self.shared.raw_id() => {
                self.handler
                    .on_event(Event::Keyboard(convert_key_press_event(&event, &self.xkb_state)));
            }

            XEvent::KeyRelease(event) if event.event == self.shared.raw_id() => {
                self.handler
                    .on_event(Event::Keyboard(convert_key_release_event(&event, &self.xkb_state)));
            }

            XEvent::FocusIn(event) if event.event == self.shared.raw_id() => {
                self.shared.is_focused.set(true);
                self.handler.on_event(Event::Window(WindowEvent::Focused));
            }

            XEvent::FocusOut(e) if e.event == self.shared.raw_id() => {
                self.shared.is_focused.set(false);
                self.handler.on_event(Event::Window(WindowEvent::Unfocused));
            }

            XEvent::ReparentNotify(e) if e.window == self.shared.raw_id() => {
                self.sizing_state.handle_parent_notify(e)
            }

            XEvent::MapNotify(e) if e.window == self.shared.raw_id() => {
                self.present_state.handle_window_mapped(
                    &self.shared.present_state,
                    &self.shared.xcb_window,
                    &self.host,
                )?;
            }

            XEvent::PresentCompleteNotify(e) if e.window == self.shared.raw_id() => {
                self.present_state.handle_present_complete_notify(e);
            }
            XEvent::Expose(e) if e.window == self.shared.raw_id() => {
                self.present_state.handle_expose_event(
                    e,
                    &self.handler,
                    &self.shared,
                    &self.sizing_state,
                );
            }

            _ => {}
        }

        Ok(())
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        // Deinit timers & FD watchers
        self.shared.timer_manager.destroy_all(&self.host);
        self.shared.present_state.destroy_redraw_timers(&self.host);

        if self.fd_registered.get() {
            let Some(fd_support) = self.host.fd_support() else { unreachable!() };
            fd_support.unregister_fd(self.shared.connection.conn.as_raw_fd()).unwrap();
            // TODO: unwrap
        }
    }
}
