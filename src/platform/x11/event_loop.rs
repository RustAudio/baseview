use super::drag_n_drop::DragNDropState;
use super::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};
use super::*;
use std::result::Result;

use crate::platform::x11::window_thread::WindowThreadShared;
use crate::warn;
use crate::wrappers::xkbcommon::XkbcommonState;
use crate::{Event, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowHandler, WindowSize};
use calloop::generic::Generic;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{Interest, LoopSignal, Mode, PostAction};
use dpi::{PhysicalPosition, PhysicalSize};
use std::rc::Rc;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::errors::ConnectionError;
use x11rb::protocol::Event as XEvent;

pub(crate) struct EventLoop {
    handler: Box<dyn WindowHandler>,
    window: Rc<WindowInner>,

    new_physical_size: Option<PhysicalSize<u16>>,

    loop_signal: LoopSignal,

    drag_n_drop: DragNDropState,
    xkb_state: Option<XkbcommonState>,

    run_error: Option<Error>,
    pub(crate) shared: Arc<WindowThreadShared>,
}

const FRAME_INTERVAL: Duration = Duration::from_millis(15);

impl EventLoop {
    pub fn new(
        window: Rc<WindowInner>, handler: Box<dyn WindowHandler>, shared: Arc<WindowThreadShared>,
        inner: &mut calloop::EventLoop<'static, Self>,
    ) -> Result<Self, Error> {
        let loop_handle = inner.handle();

        loop_handle
            .insert_source(Timer::from_duration(FRAME_INTERVAL), |i, _, e| e.handle_frame(i))
            .map_err(|e| e.error)?;

        loop_handle
            .insert_source(
                Generic::new_with_error(window.connection.conn.clone(), Interest::READ, Mode::Edge),
                |_, _, e| e.handle_connection_event_ready(),
            )
            .map_err(|e| e.error)?;

        Ok(Self {
            loop_signal: inner.get_signal(),
            handler,
            new_physical_size: None,
            drag_n_drop: DragNDropState::NoCurrentSession,
            xkb_state: XkbcommonState::new(&window.connection),
            run_error: None,
            shared,
            window,
        })
    }

    pub fn window_id(&self) -> NonZeroU32 {
        self.window.xcb_window.id()
    }

    #[inline]
    fn drain_xcb_events(&mut self) -> Result<(), ConnectionError> {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_physical_size = None;

        while let Some(event) = self.window.connection.conn.poll_for_event()? {
            self.handle_xcb_event(event)?;
        }

        if let Some(size) = self.new_physical_size.take() {
            let previous = self.window.window_size.replace(size);

            let scale_factor = self.window.scaling_factor.get();
            if let Err(e) =
                self.handler.resized(WindowSize::from_physical(size.cast(), scale_factor))
            {
                warn!("Window Handler failed to resize: {}", e);
                self.window.window_size.set(previous);
                self.window.xcb_window.resize(previous.cast())?.check_warn();
            }
        }

        Ok(())
    }

    fn handle_connection_event_ready(&mut self) -> Result<PostAction, ConnectionError> {
        self.drain_xcb_events()?;

        Ok(PostAction::Continue)
    }

    fn handle_frame(&mut self, previous_deadline: Instant) -> TimeoutAction {
        if let Err(e) = self.handler.on_frame() {
            self.run_error = Some(e.into());
            self.loop_signal.stop();
            return TimeoutAction::Drop;
        }

        // We'll try to keep a consistent frame pace. If the last frame couldn't be processed in
        // the expected frame time, this will throttle down to prevent multiple frames from
        // being queued up.

        let now = Instant::now();
        let next_deadline = if previous_deadline + FRAME_INTERVAL >= now {
            now + FRAME_INTERVAL
        } else {
            previous_deadline + FRAME_INTERVAL
        };

        TimeoutAction::ToInstant(next_deadline)
    }

    fn handle_idle(&mut self) {
        // Check for any events in the internal buffers before going to sleep:
        let _ = self.drain_xcb_events();
    }

    pub fn run(mut self, mut inner: calloop::EventLoop<Self>) -> Result<(), Error> {
        inner.run(None, &mut self, Self::handle_idle)?;

        self.handle_event(Event::Window(WindowEvent::WillClose));

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
            XEvent::ClientMessage(event) => {
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

            XEvent::ConfigureNotify(event) => {
                let new_physical_size = PhysicalSize::new(event.width, event.height);

                if self.new_physical_size.is_some()
                    || new_physical_size != self.window.window_size.get()
                {
                    self.new_physical_size = Some(new_physical_size);
                }
            }

            ////
            // mouse
            ////
            XEvent::MotionNotify(event) => {
                let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);

                self.handle_event(Event::Mouse(MouseEvent::CursorMoved {
                    position: physical_pos.cast(),
                    modifiers: key_mods(event.state),
                }));
            }

            XEvent::EnterNotify(event) => {
                self.handle_event(Event::Mouse(MouseEvent::CursorEntered));
                // since no `MOTION_NOTIFY` event is generated when `ENTER_NOTIFY` is generated,
                // we generate a CursorMoved as well, so the mouse position from here isn't lost
                let physical_pos = PhysicalPosition::new(event.event_x, event.event_y);
                self.handle_event(Event::Mouse(MouseEvent::CursorMoved {
                    position: physical_pos.cast(),
                    modifiers: key_mods(event.state),
                }));
            }

            XEvent::LeaveNotify(_) => {
                self.handle_event(Event::Mouse(MouseEvent::CursorLeft));
            }

            XEvent::ButtonPress(event) => match event.detail {
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
            },

            XEvent::ButtonRelease(event) if !(4..=7).contains(&event.detail) => {
                let button_id = mouse_id(event.detail);
                self.handle_event(Event::Mouse(MouseEvent::ButtonReleased {
                    button: button_id,
                    modifiers: key_mods(event.state),
                }));
            }

            ////
            // keys
            ////
            XEvent::KeyPress(event) => {
                let ev = Event::Keyboard(convert_key_press_event(&event, &mut self.xkb_state));
                self.handle_event(ev);
            }

            XEvent::KeyRelease(event) => {
                let ev = Event::Keyboard(convert_key_release_event(&event, &mut self.xkb_state));
                self.handle_event(ev);
            }

            XEvent::FocusIn(_) => {
                self.window.is_focused.set(true);
                self.handle_event(Event::Window(WindowEvent::Focused));
            }

            XEvent::FocusOut(_) => {
                self.window.is_focused.set(false);
                self.handle_event(Event::Window(WindowEvent::Unfocused));
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
