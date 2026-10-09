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
use calloop::PostAction;
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
}

impl EventLoop {
    pub fn waker(&self) -> WindowWaker {
        todo!()
    }

    pub fn request_poll(&self) -> PlatformResult<()> {
        todo!()
    }

    pub fn sizing_strategy(&self) -> SizingStrategy {
        todo!()
    }

    pub fn handle_main_thread_callback(&self) {
        todo!()
    }

    pub fn max_size(&self) -> Option<Size> {
        todo!()
    }

    pub fn min_size(&self) -> Option<Size> {
        todo!()
    }

    pub fn is_resizable(&self) -> bool {
        todo!()
    }

    pub fn is_open(&self) -> bool {
        todo!()
    }

    pub fn run_until_closed(&self) -> PlatformResult<()> {
        self.show()?;

        let connection = Rc::clone(&self.shared.connection);
        let mut poller = ConnectionPoller::new(&connection.conn)?;

        let mut last_frame = Instant::now();
        self.shared.stop_own_event_loop.set(false);

        while !self.shared.stop_own_event_loop.get() {
            // TODO: handle timers

            // Check for any events in the internal buffers
            // before going to sleep:
            self.handle_idle();

            eprintln!("WAITING...");
            // FIXME: handle errors
            if let PollStatus::ReadAvailable = dbg!(poller.wait())? {
                self.drain_xcb_events()?;
            }
            /*
            // Check if the user has requested the window to close
            if self.window.close_requested.get() {
                self.handle_must_close();
                self.window.close_requested.set(false);
            }*/
        }

        poller.delete()?;

        Ok(())
    }

    pub fn size(&self) -> WindowSize {
        todo!()
    }
}

impl EventLoop {
    pub fn create_window(init: WindowInitializer) -> PlatformResult<Self> {
        let parent_id = init.settings.parent.as_ref().map(|p| p.inner.window_id);
        let loop_handle = HostHandle::new(init.host);
        let shared = WindowShared::create(init.settings, Rc::clone(&loop_handle))?;
        let handler = init.builder.build(WindowContext::new(Rc::clone(&shared)))?;

        Ok(Self {
            handler: Handler::new(handler),
            present_state: PresentState::new(),
            sizing_state: SizingState::new(parent_id),

            drag_n_drop: DragNDropState::NoCurrentSession,
            xkb_state: XkbcommonState::new(&shared.connection),
            run_error: None,

            shared,
            host: loop_handle,
        })
    }

    pub(crate) fn handle_timer(&mut self, handle: &TimerHandle) {
        self.handler.on_timer(handle.into())
    }

    #[inline]
    fn drain_xcb_events(&self) -> Result<bool, FatalError> {
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

    fn handle_connection_event_ready(&mut self) -> Result<PostAction, FatalError> {
        self.drain_xcb_events()?;

        Ok(PostAction::Continue)
    }

    fn handle_idle(&self) {
        if let Err(e) = self.try_handle_idle() {
            self.trigger_fatal_error(e.into());
        }
    }

    fn try_handle_idle(&self) -> Result<(), FatalError> {
        // Check for any events in the internal buffers before going to sleep:
        self.drain_xcb_events()?;

        loop {
            self.sizing_state.handle_coalesced_resize_events(
                &self.shared,
                &self.handler,
                &self.host,
            )?;

            self.present_state.handle_requests(&self.shared, &self.handler, &self.host)?;

            if !self.drain_xcb_events()? {
                break;
            }
        }

        self.shared.connection.conn.flush()?;

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

                todo!();
                /*
                if event.type_ == self.shared.connection.atoms.XdndEnter {
                    self.drag_n_drop.handle_enter_event(&self.shared, &self.handler, &event)?;
                } else if event.type_ == self.shared.connection.atoms.XdndPosition {
                    self.drag_n_drop.handle_position_event(&self.shared, &self.handler, &event)?;
                } else if event.type_ == self.shared.connection.atoms.XdndDrop {
                    self.drag_n_drop.handle_drop_event(&self.shared, &self.handler, &event)?;
                } else if event.type_ == self.shared.connection.atoms.XdndLeave {
                    self.drag_n_drop.handle_leave_event(&self.handler, &event);
                }*/
            }

            XEvent::SelectionNotify(event) => {
                todo!()
                /*
                if event.property == self.shared.connection.atoms.XdndSelection {
                    self.drag_n_drop.handle_selection_notify_event(
                        &self.shared,
                        &self.handler,
                        &event,
                    )?;
                }*/
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
