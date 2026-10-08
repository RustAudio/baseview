use super::drag_n_drop::DragNDropState;
use super::keyboard::{convert_key_press_event, convert_key_release_event};
use super::prelude::*;
use crate::host::HostMainThreadCaller;
use crate::platform::x11::error::FatalError;
use crate::platform::x11::handler::Handler;
use crate::platform::x11::present::PresentState;
use crate::platform::x11::sizing::SizingState;
use crate::platform::x11::window_thread::{
    HostCallback, WindowThreadRequest, WindowThreadResponseMessage,
};
use crate::warn;
use crate::wrappers::xkbcommon::XkbcommonState;
use crate::{Event, WindowEvent, WindowHandler};
use calloop::generic::Generic;
use calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction};
use std::num::NonZeroU32;
use std::result::Result;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use x11rb::connection::Connection;
use x11rb::protocol::Event as XEvent;

pub struct MainThreadCaller {
    sender: mpsc::Sender<HostCallback>,
    caller: Box<dyn HostMainThreadCaller>,
}

impl MainThreadCaller {
    pub(crate) fn new(
        main_thread: Option<Box<dyn HostMainThreadCaller>>,
    ) -> (Option<Self>, Option<Receiver<HostCallback>>) {
        let Some(main_thread) = main_thread else {
            return (None, None);
        };

        let (sender, receiver) = mpsc::channel();
        (Some(Self { sender, caller: main_thread }), Some(receiver))
    }

    pub fn send(&mut self, msg: HostCallback) -> Result<(), FatalError> {
        self.sender.send(msg).map_err(|_| FatalError::SendMainThread)?;
        self.caller.call_main_thread();
        Ok(())
    }
}

pub(crate) struct EventLoop {
    response_sender: mpsc::Sender<WindowThreadResponseMessage>,
    main_thread: Option<MainThreadCaller>,

    handler: Handler,
    shared: Rc<WindowShared>,

    drag_n_drop: DragNDropState,
    sizing_state: SizingState,
    pub present_state: PresentState,
    xkb_state: Option<XkbcommonState>,

    loop_signal: LoopSignal,
    loop_handle: LoopHandle<'static, Self>,

    run_error: Option<PlatformError>,
}

impl EventLoop {
    pub fn new(
        window: Rc<WindowShared>, handler: Box<dyn WindowHandler>, parent_id: Option<NonZeroU32>,
        request_receiver: calloop::channel::Channel<WindowThreadRequest>,
        response_sender: mpsc::Sender<WindowThreadResponseMessage>,
        main_thread: Option<MainThreadCaller>, inner: &mut calloop::EventLoop<'static, Self>,
    ) -> Result<Self, PlatformError> {
        let loop_handle = inner.handle();

        loop_handle
            .insert_source(
                Generic::new_with_error(
                    Arc::clone(&window.connection.conn),
                    Interest::READ,
                    Mode::Edge,
                ),
                |_, _, e| e.handle_connection_event_ready(),
            )
            .map_err(|e| e.error)?;

        loop_handle
            .insert_source(request_receiver, |e, _, l| l.handle_main_thread_request(e))
            .map_err(|e| e.error)?;

        Ok(Self {
            loop_signal: inner.get_signal(),
            loop_handle,
            handler: Handler::new(handler),
            present_state: PresentState::new(),
            sizing_state: SizingState::new(parent_id),

            drag_n_drop: DragNDropState::NoCurrentSession,
            xkb_state: XkbcommonState::new(&window.connection),
            run_error: None,
            main_thread,

            shared: window,
            response_sender,
        })
    }

    pub(crate) fn handle_timer(&mut self, handle: &TimerHandle) {
        self.handler.on_timer(handle.into())
    }

    #[inline]
    fn drain_xcb_events(&mut self) -> Result<bool, FatalError> {
        let mut event_received = false;
        while let Some(event) = self.shared.connection.conn.poll_for_event()? {
            event_received = true;
            self.handle_xcb_event(event)?;
        }

        Ok(event_received)
    }

    fn handle_main_thread_request(&mut self, event: calloop::channel::Event<WindowThreadRequest>) {
        match event {
            calloop::channel::Event::Closed => {
                // Closed channel means the sender, i.e. the Window Handle has been dropped.
                // It should already stop this event loop on drop, but we'll take the hint.
                self.stop_now();
            }
            calloop::channel::Event::Msg(req) => match self.handle_request(req) {
                Ok(()) => self.send_response(Ok(())),
                Err(e) => self.send_response(Err(e.to_string())),
            },
        }
    }

    fn send_response(&mut self, response: WindowThreadResponseMessage) {
        if let Err(e) = self.response_sender.send(response) {
            warn!("Failed to send response back to main thread: {}", &e);
            if let Err(e) = e.0 {
                crate::error!("Request failed: {}", e)
            }

            self.stop_now();
        }
    }

    pub fn stop_now(&self) {
        self.loop_signal.stop();
        self.loop_signal.wakeup();
    }

    pub fn trigger_fatal_error(&mut self, error: PlatformError) {
        if self.run_error.is_none() {
            self.run_error = Some(error);
        }
        self.stop_now();
    }

    fn handle_request(&mut self, req: WindowThreadRequest) -> Result<(), PlatformError> {
        match req {
            WindowThreadRequest::Resize(new_size) => {
                self.sizing_state.handle_host_resize(new_size, &self.handler, &self.shared)?
            }
            WindowThreadRequest::SuggestScaleFactor(scale) => self
                .sizing_state
                .handle_host_suggest_scale_factor(scale, &self.handler, &self.shared)?,
            WindowThreadRequest::SetParent(new_parent) => {
                self.shared.xcb_window.reparent(Some(new_parent.window_id))?.check()?
            }
            WindowThreadRequest::Show => self.shared.xcb_window.map_window()?.check()?,
            WindowThreadRequest::Hide => self.shared.xcb_window.unmap_window()?.check()?,
        }

        Ok(())
    }

    fn handle_connection_event_ready(&mut self) -> Result<PostAction, FatalError> {
        self.drain_xcb_events()?;

        Ok(PostAction::Continue)
    }

    fn handle_idle(&mut self) {
        if let Err(e) = self.try_handle_idle() {
            self.trigger_fatal_error(e.into());
        }
    }

    fn try_handle_idle(&mut self) -> Result<(), FatalError> {
        // Check for any events in the internal buffers before going to sleep:
        self.drain_xcb_events()?;

        loop {
            self.sizing_state.handle_coalesced_resize_events(
                &self.shared,
                &self.handler,
                self.main_thread.as_mut(),
            )?;

            self.present_state.handle_requests(&self.shared, &self.handler, &self.loop_handle)?;

            if !self.drain_xcb_events()? {
                break;
            }
        }

        self.shared.connection.conn.flush()?;

        Ok(())
    }

    pub fn run(mut self, mut inner: calloop::EventLoop<Self>) -> Result<(), PlatformError> {
        self.drain_xcb_events()?;
        inner.run(None, &mut self, Self::handle_idle)?;

        self.handler.on_event(Event::Window(WindowEvent::WillClose));

        // If the event loop doesn't stop because the host asked it to, then we should notify it
        if !self.shared.main_thread_shared.is_stop_host_requested() {
            if let Some(main_thread) = self.main_thread.as_mut() {
                if let Err(e) = main_thread.send(HostCallback::Destroyed) {
                    warn!("Could not notify host that X11 thread is stopping: {}", e)
                }
            }
        }

        if let Some(err) = self.run_error {
            return Err(err);
        };

        Ok(())
    }

    pub fn shared(&self) -> &WindowShared {
        &self.shared
    }

    fn handle_xcb_event(&mut self, event: XEvent) -> Result<(), FatalError> {
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
                self.handler.on_event(Event::Keyboard(convert_key_press_event(
                    &event,
                    &mut self.xkb_state,
                )));
            }

            XEvent::KeyRelease(event) if event.event == self.shared.raw_id() => {
                self.handler.on_event(Event::Keyboard(convert_key_release_event(
                    &event,
                    &mut self.xkb_state,
                )));
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
                    &self.loop_handle,
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
