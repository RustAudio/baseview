use super::drag_n_drop::DragNDropState;
use super::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};
use super::*;
use std::result::Result;

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::host::HostMainThreadCaller;
use crate::platform::x11::error::FatalError;
use crate::platform::x11::handler::Handler;
use crate::platform::x11::present::PresentState;
use crate::platform::x11::window_thread::{
    HostCallback, WindowThreadRequest, WindowThreadResponseMessage,
};
use crate::warn;
use crate::wrappers::xkbcommon::XkbcommonState;
use crate::{Event, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowHandler, WindowSize};
use calloop::generic::Generic;
use calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction};
use std::rc::Rc;
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
    handler: Handler,
    shared: Rc<WindowShared>,

    new_size: Option<PhysicalSize<u16>>,
    new_parent_size: Option<PhysicalSize<u16>>,

    pub present_state: PresentState,
    loop_signal: LoopSignal,
    loop_handle: LoopHandle<'static, Self>,

    drag_n_drop: DragNDropState,
    xkb_state: Option<XkbcommonState>,

    run_error: Option<PlatformError>,

    response_sender: mpsc::Sender<WindowThreadResponseMessage>,
    main_thread: Option<MainThreadCaller>,
}

impl EventLoop {
    pub fn new(
        window: Rc<WindowShared>, handler: Box<dyn WindowHandler>,
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
            new_size: None,
            new_parent_size: None,
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

    fn handle_coalesced_resize_events(&mut self) -> Result<(), FatalError> {
        let mut comes_from_parent = false;
        if let Some(new_parent_size) = self.new_parent_size.take() {
            if new_parent_size != self.shared.get_size() {
                // The parent was resized, which means we should resize ourselves too.
                if let Err(e) = self.shared.xcb_window.resize(new_parent_size.cast()) {
                    crate::warn!("Failed to resize window: {}", e);
                } else {
                    // Makes the rest of this function run on the new parent size immediately (without waiting for a ConfigureNotify round-trip)
                    // Also overrides any new sizes we may have received this event loop iteration,it would probably be invalidated anyway
                    self.new_size = Some(new_parent_size);
                    comes_from_parent = true;
                }
            }
        }

        let Some(new_size) = self.new_size.take() else { return Ok(()) };
        let previous = self.shared.store_size(new_size);

        if previous == new_size {
            return Ok(());
        };

        let scale_factor = self.shared.scaling_factor.get();
        let new_size = WindowSize::from_physical(new_size.cast(), scale_factor);

        if let Err(()) = self.handler.resize(new_size) {
            self.shared.store_size(previous);
            self.shared.xcb_window.resize(previous.cast())?.check_warn();
            return Ok(());
        }

        // Host requests use resize_immediately, which stops the previous == new_size condition
        // So if we're here, it's guaranteed not to be from a host request

        if !comes_from_parent {
            if let Some(host) = self.main_thread.as_mut() {
                host.send(HostCallback::Resized {
                    new_size,
                    previous: WindowSize::from_physical(previous.cast(), scale_factor),
                })?;
            }
        }

        // Immediately schedule a redraw, do not wait for an "expose" event
        self.shared.request_redraw();

        Ok(())
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
                let scale_factor = self.shared.scaling_factor.get();
                let new_size = new_size.to_physical(scale_factor);

                self.shared.resize_immediately(new_size, &self.handler)?;

                Ok(())
            }
            WindowThreadRequest::SuggestScaleFactor(scale) => {
                // If the scaling factor is already provided by the system, do nothing
                if !self.shared.scaling_factor.suggest(scale) {
                    return Ok(());
                };

                let current_logical_size = self.shared.get_size().to_logical::<f64>(1.0);
                let new_physical_size = current_logical_size.to_physical(scale);

                self.shared.resize_immediately(new_physical_size, &self.handler)?;

                Ok(())
            }
            WindowThreadRequest::SetParent(new_parent) => {
                self.shared.xcb_window.reparent(Some(new_parent.window_id))?;

                Ok(())
            }
            WindowThreadRequest::Show => {
                self.shared.xcb_window.map_window()?.check()?;
                Ok(())
            }
            WindowThreadRequest::Hide => {
                self.shared.xcb_window.unmap_window()?.check()?;
                Ok(())
            }
        }
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
            self.handle_coalesced_resize_events()?;

            // Consume all requests from above poll
            if let Some(redraw_after) = self.shared.main_thread_shared.take_redraw_request() {
                self.shared.request_redraw_after(redraw_after)
            }

            let shared_poll_requested = self.shared.main_thread_shared.take_poll_request();
            let did_redraw = self.present_state.redraw_if_needed(
                &self.shared.present_state,
                &mut self.handler,
                &self.shared.connection.conn,
            )?;

            if !did_redraw {
                if shared_poll_requested || self.shared.poll_requested.get() {
                    self.handler.poll();
                    self.shared.poll_requested.set(false);
                }
            }

            self.present_state.handle_present_notify(
                &self.shared.present_state,
                &self.shared.xcb_window,
                &self.loop_handle,
            )?;

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

        self.handle_event(Event::Window(WindowEvent::WillClose));

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

    pub fn request_redraw(&self) {
        self.shared.request_redraw();
    }

    pub fn shared(&self) -> &WindowShared {
        &self.shared
    }

    fn handle_xcb_event(&mut self, event: XEvent) -> Result<(), FatalError> {
        // For all the keyboard and mouse events, you can fetch
        // `x`, `y`, `detail`, and `state`.
        // - `x` and `y` are the position inside the window where the cursor currently is
        //   when the event happened.
        // - `detail` will tell you which keycode was pressed/released (for keyboard events)
        //   or which mouse button was pressed/released (for mouse events).
        //   For mouse events, here's what the value means (at least on my current mouse):
        //      1 = left mouse button
        //      2 = middle mouse button (scroll wheel)
        //      3 = right mouse button
        //      4 = scroll wheel up
        //      5 = scroll wheel down
        //      8 = lower side button ("back" button)
        //      9 = upper side button ("forward" button)
        //   Note that you *will* get a "button released" event for even the scroll wheel
        //   events, which you can probably ignore.
        // - `state` will tell you the state of the main three mouse buttons and some of
        //   the keyboard modifier keys at the time of the event.
        //   http://rtbo.github.io/rust-xcb/src/xcb/ffi/xproto.rs.html#445

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
                if let Some(window_id) = NonZero::new(event.window) {
                    // These are coalesced and then handled asynchronously at the end of the event loop
                    if window_id == self.shared.xcb_window.id() {
                        self.new_size = Some(PhysicalSize::new(event.width, event.height));
                    } else if Some(window_id) == self.shared.parent_id.get() {
                        // Also resize the window if the parent is resized
                        // This works around some hosts that might not call set_size() right away (or at all...)
                        self.new_parent_size = Some(PhysicalSize::new(event.width, event.height));
                    }
                }
            }

            ////
            // mouse
            ////
            XEvent::MotionNotify(event) if event.event == self.shared.raw_id() => {
                let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);

                self.handle_event(Event::Mouse(MouseEvent::CursorMoved {
                    position: physical_pos.cast(),
                    modifiers: key_mods(event.state),
                }));
            }

            XEvent::EnterNotify(event) if event.event == self.shared.raw_id() => {
                self.handle_event(Event::Mouse(MouseEvent::CursorEntered));
                // since no `MOTION_NOTIFY` event is generated when `ENTER_NOTIFY` is generated,
                // we generate a CursorMoved as well, so the mouse position from here isn't lost
                let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);
                self.handle_event(Event::Mouse(MouseEvent::CursorMoved {
                    position: physical_pos.cast(),
                    modifiers: key_mods(event.state),
                }));
            }

            XEvent::LeaveNotify(event) if event.event == self.shared.raw_id() => {
                self.handle_event(Event::Mouse(MouseEvent::CursorLeft));
            }

            XEvent::ButtonPress(event) if event.event == self.shared.raw_id() => {
                match event.detail {
                    4..=7 => {
                        self.handle_event(Event::Mouse(MouseEvent::WheelScrolled {
                            delta: match event.detail {
                                4 => ScrollDelta::Lines { x: 0.0, y: 1.0 },
                                5 => ScrollDelta::Lines { x: 0.0, y: -1.0 },
                                6 => ScrollDelta::Lines { x: -1.0, y: 0.0 },
                                7 => ScrollDelta::Lines { x: 1.0, y: 0.0 },
                                _ => unreachable!(),
                            },
                            modifiers: key_mods(event.state),
                        }));
                    }
                    detail => {
                        self.handle_event(Event::Mouse(MouseEvent::ButtonPressed {
                            button: mouse_id(detail),
                            modifiers: key_mods(event.state),
                        }));
                    }
                }
            }

            XEvent::ButtonRelease(event)
                if event.event == self.shared.raw_id() && !(4..=7).contains(&event.detail) =>
            {
                let button_id = mouse_id(event.detail);
                self.handle_event(Event::Mouse(MouseEvent::ButtonReleased {
                    button: button_id,
                    modifiers: key_mods(event.state),
                }));
            }

            ////
            // keys
            ////
            XEvent::KeyPress(event) if event.event == self.shared.raw_id() => {
                let ev = Event::Keyboard(convert_key_press_event(&event, &mut self.xkb_state));
                self.handle_event(ev);
            }

            XEvent::KeyRelease(event) if event.event == self.shared.raw_id() => {
                let ev = Event::Keyboard(convert_key_release_event(&event, &mut self.xkb_state));
                self.handle_event(ev);
            }

            XEvent::FocusIn(event) if event.event == self.shared.raw_id() => {
                self.shared.is_focused.set(true);
                self.handle_event(Event::Window(WindowEvent::Focused));
            }

            XEvent::FocusOut(e) if e.event == self.shared.raw_id() => {
                self.shared.is_focused.set(false);
                self.handle_event(Event::Window(WindowEvent::Unfocused));
            }

            XEvent::ReparentNotify(e) if e.window == self.shared.raw_id() => {
                self.shared.parent_id.set(NonZero::new(e.parent));
            }

            XEvent::MapNotify(e) => {
                if let Some(window_id) = NonZero::new(e.window) {
                    if window_id == self.shared.xcb_window.id() {
                        self.present_state.handle_window_mapped(
                            &self.shared.present_state,
                            &self.shared.xcb_window,
                            &self.loop_handle,
                        )?;
                    }
                }
            }

            XEvent::PresentCompleteNotify(e) if e.window != self.shared.raw_id() => {
                self.present_state.handle_present_complete_notify(e);
            }
            XEvent::Expose(e) if e.window == self.shared.raw_id() => {
                self.present_state.handle_expose_event(
                    e,
                    &self.shared.present_state,
                    &self.handler,
                );
            }

            _ => {}
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        self.handler.on_event(event);
    }
}

fn mouse_id(id: u8) -> MouseButton {
    match id {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        8 => MouseButton::Back,
        9 => MouseButton::Forward,
        id => MouseButton::Other(id),
    }
}
