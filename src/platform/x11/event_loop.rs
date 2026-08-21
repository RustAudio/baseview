use super::drag_n_drop::DragNDropState;
use super::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};
use super::*;
use std::result::Result;

use crate::host::HostMainThreadCaller;
use crate::platform::x11::error::FatalError;
use crate::platform::x11::window_thread::{
    HostCallback, WindowThreadRequest, WindowThreadResponseMessage,
};
use crate::warn;
use crate::wrappers::xkbcommon::XkbcommonState;
use crate::{Event, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowHandler, WindowSize};
use calloop::generic::Generic;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction};
use dpi::{PhysicalPosition, PhysicalSize};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::errors::ConnectionError;
use x11rb::protocol::present::CompleteKind;
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
    handler: Box<dyn WindowHandler>,
    window: Rc<WindowInner>,

    new_size: Option<PhysicalSize<u16>>,
    new_parent_size: Option<PhysicalSize<u16>>,
    draw_now: bool,
    last_requested_serial: Option<u32>,
    last_received_present: Option<(u32, u64)>,

    loop_signal: LoopSignal,

    drag_n_drop: DragNDropState,
    xkb_state: Option<XkbcommonState>,

    run_error: Option<PlatformError>,

    response_sender: mpsc::Sender<WindowThreadResponseMessage>,
    main_thread: Option<MainThreadCaller>,
}

impl EventLoop {
    pub fn new(
        window: Rc<WindowInner>, handler: Box<dyn WindowHandler>,
        request_receiver: calloop::channel::Channel<WindowThreadRequest>,
        response_sender: mpsc::Sender<WindowThreadResponseMessage>,
        main_thread: Option<MainThreadCaller>, inner: &mut calloop::EventLoop<'static, Self>,
    ) -> Result<Self, PlatformError> {
        let loop_handle = inner.handle();

        // Self::setup_fallback_frame_timer(&loop_handle)?;

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
            handler,
            new_size: None,
            new_parent_size: None,
            draw_now: false,
            last_requested_serial: None,
            last_received_present: None,
            drag_n_drop: DragNDropState::NoCurrentSession,
            xkb_state: XkbcommonState::new(&window.connection),
            run_error: None,
            main_thread,

            window,
            response_sender,
        })
    }

    #[inline]
    fn drain_xcb_events(&mut self) -> Result<bool, ConnectionError> {
        let mut event_received = false;
        while let Some(event) = self.window.connection.conn.poll_for_event()? {
            event_received = true;
            self.handle_xcb_event(event)?;
        }

        Ok(event_received)
    }

    fn setup_fallback_frame_timer(
        loop_handle: &LoopHandle<'_, Self>,
    ) -> Result<(), calloop::Error> {
        const FRAME_INTERVAL: Duration = Duration::from_millis(15);

        fn handle_frame(evloop: &mut EventLoop, previous_deadline: Instant) -> TimeoutAction {
            evloop.draw_now = true;

            // We'll try to keep a consistent frame pace. If the last frame couldn't be processed in
            // the expected frame time, this will throttle down to prevent multiple frames from
            // being queued up.

            let now = Instant::now();

            let Some(next_deadline) = previous_deadline.checked_add(FRAME_INTERVAL) else {
                return TimeoutAction::ToDuration(FRAME_INTERVAL);
            };

            if next_deadline >= now {
                return TimeoutAction::ToDuration(FRAME_INTERVAL);
            }

            TimeoutAction::ToInstant(next_deadline)
        }

        loop_handle
            .insert_source(Timer::from_duration(FRAME_INTERVAL), |i, _, e| handle_frame(e, i))
            .map_err(|e| e.error)?;

        Ok(())
    }

    fn handle_redraw(&mut self) {
        if !self.draw_now {
            return;
        }
        self.draw_now = false;

        if !self.window.visibility_state.own_window_is_viewable() {
            return;
        }

        if let Err(e) = self.handler.draw() {
            self.trigger_fatal_error(e.into());
            return;
        }

        self.window.present_notify_requested.set(true);

        // Any socket error will be handled in the next poll
        let _ = self.window.connection.conn.flush();
    }

    fn handle_present_notify(&mut self) -> Result<(), ConnectionError> {
        if !self.window.present_notify_requested.get() {
            return Ok(());
        }

        let (next_serial, target_msc) =
            match (self.last_requested_serial, self.last_received_present) {
                // First request, always send
                (None, None) => (0, 0),
                (Some(sent_serial), Some((received_serial, last_msc)))
                    if sent_serial == received_serial =>
                {
                    // TODO: why does 2 work but not 1 for next MSC??
                    (sent_serial.wrapping_add(1), last_msc.wrapping_add(2))
                }
                // We sent our first request but have not gotten a response yet.
                // Or, we sent a request, but the last response we've gotten isn't that one.
                // Do not send.
                _ => {
                    self.window.present_notify_requested.set(false);
                    return Ok(());
                }
            };

        self.window.xcb_window.present_notify(target_msc, next_serial)?.check_warn(); // TODO: handle error
        self.last_requested_serial = Some(next_serial);
        self.window.present_notify_requested.set(false);

        Ok(())
    }

    fn handle_coalesced_resize_events(&mut self) -> Result<(), FatalError> {
        if let Some(new_parent_size) = self.new_parent_size.take() {
            if new_parent_size != self.window.get_size() {
                // The parent was resized, which means we should resize ourselves too.
                if let Err(e) = self.window.xcb_window.resize(new_parent_size.cast()) {
                    crate::warn!("Failed to resize window: {}", e);
                } else {
                    // Makes the rest of this function run on the new parent size immediately (without waiting for a ConfigureNotify round-trip)
                    // Also overrides any new sizes we may have received this event loop iteration,it would probably be invalidated anyway
                    self.new_size = Some(new_parent_size);
                }
            }
        }

        let Some(new_size) = self.new_size.take() else { return Ok(()) };
        let previous = self.window.store_size(new_size);

        if previous == new_size {
            return Ok(());
        };

        let scale_factor = self.window.scaling_factor.get();
        let new_size = WindowSize::from_physical(new_size.cast(), scale_factor);

        if let Err(e) = self.handler.resized(new_size) {
            warn!("Window Handler failed to resize: {}", e);
            self.window.store_size(previous);
            self.window.xcb_window.resize(previous.cast())?.check_warn();
        } else {
            // Host requests use resize_immediately, which stops the previous == new_size condition
            // So if we're here, it's guaranteed not to be from a host request

            if let Some(host) = self.main_thread.as_mut() {
                host.send(HostCallback::Resized {
                    new_size,
                    previous: WindowSize::from_physical(previous.cast(), scale_factor),
                })?;
            }

            // Immediately schedule a redraw, do not wait for an "expose" event
            self.window.present_notify_requested.set(true);
        }

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

    fn stop_now(&self) {
        self.loop_signal.stop();
        self.loop_signal.wakeup();
    }

    fn trigger_fatal_error(&mut self, error: PlatformError) {
        if self.run_error.is_none() {
            self.run_error = Some(error);
        }
        self.stop_now();
    }

    fn handle_request(&mut self, req: WindowThreadRequest) -> Result<(), PlatformError> {
        match req {
            WindowThreadRequest::Resize(new_size) => {
                let scale_factor = self.window.scaling_factor.get();
                let new_size = new_size.to_physical(scale_factor);

                self.window.resize_immediately(new_size, &*self.handler)?;

                Ok(())
            }
            WindowThreadRequest::SuggestScaleFactor(scale) => {
                // If the scaling factor is already provided by the system, do nothing
                if !self.window.scaling_factor.suggest(scale) {
                    return Ok(());
                };

                let current_logical_size = self.window.get_size().to_logical::<f64>(1.0);
                let new_physical_size = current_logical_size.to_physical(scale);

                self.window.resize_immediately(new_physical_size, &*self.handler)?;

                Ok(())
            }
            WindowThreadRequest::SetParent(new_parent) => {
                self.window.xcb_window.reparent(Some(new_parent.window_id))?;

                Ok(())
            }
            WindowThreadRequest::Show => {
                self.window.xcb_window.map_window()?.check()?;
                self.window.visibility_state.window_mapped(self.window.xcb_window.id());
                Ok(())
            }
            WindowThreadRequest::Hide => {
                self.window.xcb_window.unmap_window()?.check()?;
                self.window.visibility_state.window_unmapped(self.window.xcb_window.id());
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
            self.handle_present_notify()?;
            self.handle_redraw();

            if !self.drain_xcb_events()? {
                break;
            }
        }

        self.window.connection.conn.flush()?;

        Ok(())
    }

    pub fn run(mut self, mut inner: calloop::EventLoop<Self>) -> Result<(), PlatformError> {
        self.drain_xcb_events()?;
        inner.run(None, &mut self, Self::handle_idle)?;

        self.handle_event(Event::Window(WindowEvent::WillClose));

        // If the event loop doesn't stop because the host asked it to, then we should notify it
        if !self.window.main_thread_shared.is_stop_host_requested() {
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

    fn handle_xcb_event(&mut self, event: XEvent) -> Result<(), ConnectionError> {
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
            ////
            // window
            ////
            XEvent::ClientMessage(event) if event.window == self.window.raw_id() => {
                if event.format != 32 {
                    return Ok(());
                }

                if event.data.as_data32()[0] == self.window.connection.atoms.WM_DELETE_WINDOW {
                    self.window.request_close();
                    return Ok(());
                }

                ////
                // drag n drop
                ////
                if event.type_ == self.window.connection.atoms.XdndEnter {
                    self.drag_n_drop.handle_enter_event(&self.window, &*self.handler, &event)?;
                } else if event.type_ == self.window.connection.atoms.XdndPosition {
                    self.drag_n_drop.handle_position_event(&self.window, &*self.handler, &event)?;
                } else if event.type_ == self.window.connection.atoms.XdndDrop {
                    self.drag_n_drop.handle_drop_event(&self.window, &*self.handler, &event)?;
                } else if event.type_ == self.window.connection.atoms.XdndLeave {
                    self.drag_n_drop.handle_leave_event(&*self.handler, &event);
                }
            }

            XEvent::SelectionNotify(event) => {
                if event.property == self.window.connection.atoms.XdndSelection {
                    self.drag_n_drop.handle_selection_notify_event(
                        &self.window,
                        &*self.handler,
                        &event,
                    )?;
                }
            }

            XEvent::Error(e) => {
                warn!("Received leftover X11 error: {:?}", e);
            }

            XEvent::ConfigureNotify(event) => {
                // These are coalesced and then handled asynchronously at the end of the event loop
                if event.window == self.window.raw_id() {
                    self.new_size = Some(PhysicalSize::new(event.width, event.height));
                } else if Some(event.window)
                    == self.window.visibility_state.parent_id().map(|i| i.get())
                {
                    // Also resize the window if the parent is resized
                    // This works around some hosts that might not call set_size() right away (or at all...)
                    self.new_parent_size = Some(PhysicalSize::new(event.width, event.height));
                }
            }

            XEvent::Expose(e) if e.window == self.window.raw_id() => {
                self.window.present_notify_requested.set(true)
            }

            ////
            // mouse
            ////
            XEvent::MotionNotify(event) if event.event == self.window.raw_id() => {
                let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);

                self.handle_event(Event::Mouse(MouseEvent::CursorMoved {
                    position: physical_pos.cast(),
                    modifiers: key_mods(event.state),
                }));
            }

            XEvent::EnterNotify(event) if event.event == self.window.raw_id() => {
                self.handle_event(Event::Mouse(MouseEvent::CursorEntered));
                // since no `MOTION_NOTIFY` event is generated when `ENTER_NOTIFY` is generated,
                // we generate a CursorMoved as well, so the mouse position from here isn't lost
                let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);
                self.handle_event(Event::Mouse(MouseEvent::CursorMoved {
                    position: physical_pos.cast(),
                    modifiers: key_mods(event.state),
                }));
            }

            XEvent::LeaveNotify(event) if event.event == self.window.raw_id() => {
                self.handle_event(Event::Mouse(MouseEvent::CursorLeft));
            }

            XEvent::ButtonPress(event) if event.event == self.window.raw_id() => {
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
                if event.event == self.window.raw_id() && !(4..=7).contains(&event.detail) =>
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
            XEvent::KeyPress(event) if event.event == self.window.raw_id() => {
                let ev = Event::Keyboard(convert_key_press_event(&event, &mut self.xkb_state));
                self.handle_event(ev);
            }

            XEvent::KeyRelease(event) if event.event == self.window.raw_id() => {
                let ev = Event::Keyboard(convert_key_release_event(&event, &mut self.xkb_state));
                self.handle_event(ev);
            }

            XEvent::FocusIn(event) if event.event == self.window.raw_id() => {
                self.window.is_focused.set(true);
                self.handle_event(Event::Window(WindowEvent::Focused));
            }

            XEvent::FocusOut(e) if e.event == self.window.raw_id() => {
                self.window.is_focused.set(false);
                self.handle_event(Event::Window(WindowEvent::Unfocused));
            }

            XEvent::MapNotify(e) => {
                if let Some(window_id) = NonZero::new(e.window) {
                    if window_id == self.window.xcb_window.id() {
                        self.window.is_mapped.set(true);
                    }

                    let became_viewable = self.window.visibility_state.window_mapped(window_id);

                    if became_viewable {
                        self.window.xcb_window.present_select_input()?.unwrap().check_warn(); // TODO: unwrap: fallback to timer
                        self.window.present_notify_requested.set(true);
                    }
                }
            }

            XEvent::UnmapNotify(e) => {
                if let Some(window_id) = NonZero::new(e.window) {
                    if window_id == self.window.xcb_window.id() {
                        self.window.is_mapped.set(false)
                    }

                    self.window.visibility_state.window_unmapped(window_id);
                }
            }

            XEvent::ReparentNotify(e) => {
                if let Some(window_id) = NonZero::new(e.window) {
                    self.window.visibility_state.window_reparented(
                        window_id,
                        NonZero::new(e.parent),
                        &self.window.connection,
                    )
                }
            }

            XEvent::DestroyNotify(e) => {
                if let Some(window_id) = NonZero::new(e.window) {
                    self.window
                        .visibility_state
                        .window_destroyed(window_id, &self.window.connection)
                }
            }

            XEvent::PresentCompleteNotify(e) => {
                if e.kind != CompleteKind::NOTIFY_MSC {
                    return Ok(());
                }

                if e.window != self.window.raw_id() {
                    dbg!(e.window);
                    return Ok(());
                }

                let Some(last_requested_serial) = self.last_requested_serial else {
                    eprintln!("Received serial without request: {}", e.serial);
                    return Ok(());
                };

                if last_requested_serial != e.serial {
                    eprintln!(
                        "Received serial not matching: {}, requested: {}",
                        e.serial, last_requested_serial
                    );
                    return Ok(());
                }

                if let Some((last_received_serial, last_received_msc)) = self.last_received_present
                {
                    if last_received_serial == e.serial {
                        eprintln!("Already handled serial: {}", last_received_serial);
                        return Ok(());
                    }

                    if e.msc <= last_received_msc {
                        eprintln!("Already handled MSC: {}", e.msc);
                        self.last_received_present = Some((e.serial, e.msc));
                        //self.window.present_notify_requested.set(true);
                        return Ok(());
                    }
                }

                self.last_received_present = Some((e.serial, e.msc));
                self.draw_now = true;
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
