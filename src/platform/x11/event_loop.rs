use super::drag_n_drop::DragNDropState;
use super::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};
use super::*;

use crate::wrappers::connection_poller::{ConnectionPoller, PollStatus};
use crate::wrappers::xkbcommon::XkbcommonState;
use crate::{Event, MouseButton, MouseEvent, ScrollDelta, WindowEvent, WindowHandler, WindowSize};
use dpi::{PhysicalPosition, PhysicalSize};
use std::error::Error;
use std::rc::Rc;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::Event as XEvent;

pub(crate) struct EventLoop {
    handler: Box<dyn WindowHandler>,
    window: Rc<WindowInner>,
    parent_handle: Option<ParentHandle>,

    new_physical_size: Option<PhysicalSize<u16>>,
    frame_interval: Duration,
    event_loop_running: bool,

    drag_n_drop: DragNDropState,

    xkb_state: Option<XkbcommonState>,
}

impl EventLoop {
    pub fn new(
        window: Rc<WindowInner>, handler: Box<dyn WindowHandler>,
        parent_handle: Option<ParentHandle>, xkb_state: Option<XkbcommonState>,
    ) -> Self {
        Self {
            window,
            handler,
            parent_handle,
            frame_interval: Duration::from_millis(15),
            event_loop_running: false,
            new_physical_size: None,
            drag_n_drop: DragNDropState::NoCurrentSession,
            xkb_state,
        }
    }

    #[inline]
    fn drain_xcb_events(&mut self) -> Result<(), Box<dyn Error>> {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_physical_size = None;

        while let Some(event) = self.window.connection.conn.poll_for_event()? {
            self.handle_xcb_event(event);
        }

        if let Some(size) = self.new_physical_size.take() {
            self.window.window_size.set(size);

            let scale_factor = self.window.scaling_factor.get();

            self.handler.resized(WindowSize::from_physical(size.cast(), scale_factor));
        }

        Ok(())
    }

    // Event loop
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let connection = Rc::clone(&self.window.connection);
        let mut poller = ConnectionPoller::new(&connection.conn)?;

        let mut last_frame = Instant::now();
        self.event_loop_running = true;

        while self.event_loop_running {
            // We'll try to keep a consistent frame pace. If the last frame couldn't be processed in
            // the expected frame time, this will throttle down to prevent multiple frames from
            // being queued up. The conditional here is needed because event handling and frame
            // drawing is interleaved. The `poll()` function below will wait until the next frame
            // can be drawn, or until the window receives an event. We thus need to manually check
            // if it's already time to draw a new frame.
            let next_frame = last_frame + self.frame_interval;
            if Instant::now() >= next_frame {
                self.handler.on_frame();
                last_frame = Instant::max(next_frame, Instant::now() - self.frame_interval);
            }

            // Check for any events in the internal buffers
            // before going to sleep:
            self.drain_xcb_events()?;

            // FIXME: handle errors
            if let PollStatus::ReadAvailable = poller.wait(next_frame).unwrap() {
                self.drain_xcb_events()?;
            }

            // Check if the parents's handle was dropped (such as when the host
            // requested the window to close)
            if let Some(parent_handle) = &self.parent_handle {
                if parent_handle.parent_did_drop() {
                    self.handle_must_close();
                    self.window.close_requested.set(false);
                }
            }

            // Check if the user has requested the window to close
            if self.window.close_requested.get() {
                self.handle_must_close();
                self.window.close_requested.set(false);
            }
        }

        poller.delete()?;

        Ok(())
    }

    fn handle_xcb_event(&mut self, event: XEvent) {
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
                    return;
                }

                if event.data.as_data32()[0] == self.window.connection.atoms.WM_DELETE_WINDOW {
                    self.handle_close_requested();
                    return;
                }

                ////
                // drag n drop
                ////
                if event.type_ == self.window.connection.atoms.XdndEnter {
                    if let Err(_e) = self.drag_n_drop.handle_enter_event(
                        &self.window,
                        &mut *self.handler,
                        &event,
                    ) {
                        // TODO: log warning
                    }
                } else if event.type_ == self.window.connection.atoms.XdndPosition {
                    if let Err(_e) = self.drag_n_drop.handle_position_event(
                        &self.window,
                        &mut *self.handler,
                        &event,
                    ) {
                        // TODO: log warning
                    }
                } else if event.type_ == self.window.connection.atoms.XdndDrop {
                    if let Err(_e) =
                        self.drag_n_drop.handle_drop_event(&self.window, &mut *self.handler, &event)
                    {
                        // TODO: log warning
                    }
                } else if event.type_ == self.window.connection.atoms.XdndLeave {
                    self.drag_n_drop.handle_leave_event(&mut *self.handler, &event);
                }
            }

            XEvent::SelectionNotify(event) => {
                if event.property == self.window.connection.atoms.XdndSelection {
                    if let Err(_e) = self.drag_n_drop.handle_selection_notify_event(
                        &self.window,
                        &mut *self.handler,
                        &event,
                    ) {
                        // TODO: Log warning
                    }
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
                    let button_id = mouse_id(detail);
                    self.handle_event(Event::Mouse(MouseEvent::ButtonPressed {
                        button: button_id,
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
    }

    fn handle_event(&mut self, event: Event) {
        self.handler.on_event(event);
    }

    fn handle_close_requested(&mut self) {
        // FIXME: handler should decide whether window stays open or not
        self.handle_must_close();
    }

    fn handle_must_close(&mut self) {
        self.handle_event(Event::Window(WindowEvent::WillClose));

        self.event_loop_running = false;
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
