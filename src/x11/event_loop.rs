use crate::x11::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};
use crate::x11::{ParentHandle, Window, WindowInner};
use crate::{
    Event, MouseButton, MouseEvent, PhyPoint, PhySize, ScrollDelta, WindowEvent, WindowHandler,
    WindowInfo,
};
use std::error::Error;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::Event as XEvent;

pub(super) struct EventLoop {
    handler: Box<dyn WindowHandler>,
    window: WindowInner,
    parent_handle: Option<ParentHandle>,

    new_physical_size: Option<PhySize>,
    frame_interval: Duration,
    event_loop_running: bool,
}

impl EventLoop {
    pub fn new(
        window: WindowInner, handler: impl WindowHandler + 'static,
        parent_handle: Option<ParentHandle>,
    ) -> Self {
        Self {
            window,
            handler: Box::new(handler),
            parent_handle,
            frame_interval: Duration::from_millis(15),
            event_loop_running: false,
            new_physical_size: None,
        }
    }

    #[inline]
    fn drain_xcb_events(&mut self) -> Result<(), Box<dyn Error>> {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_physical_size = None;

        while let Some(event) = self.window.xcb_connection.conn.poll_for_event()? {
            self.handle_xcb_event(event);
        }

        if let Some(size) = self.new_physical_size.take() {
            self.window.window_info =
                WindowInfo::from_physical_size(size, self.window.window_info.scale());

            let window_info = self.window.window_info;

            self.handler.on_event(
                &mut crate::Window::new(Window { inner: &self.window }),
                Event::Window(WindowEvent::Resized(window_info)),
            );
        }

        Ok(())
    }

    // Event loop
    // FIXME: poll() acts fine on linux, sometimes funky on *BSD. XCB upstream uses a define to
    // switch between poll() and select() (the latter of which is fine on *BSD), and we should do
    // the same.
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        use nix::poll::*;

        let xcb_fd = self.window.xcb_connection.conn.as_raw_fd();

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
                self.handler.on_frame(&mut crate::Window::new(Window { inner: &self.window }));
                last_frame = Instant::max(next_frame, Instant::now() - self.frame_interval);
            }

            let mut fds = [PollFd::new(xcb_fd, PollFlags::POLLIN)];

            // Check for any events in the internal buffers
            // before going to sleep:
            self.drain_xcb_events()?;

            // FIXME: handle errors
            poll(&mut fds, next_frame.duration_since(Instant::now()).subsec_millis() as i32)
                .unwrap();

            if let Some(revents) = fds[0].revents() {
                if revents.contains(PollFlags::POLLERR) {
                    panic!("xcb connection poll error");
                }

                if revents.contains(PollFlags::POLLIN) {
                    self.drain_xcb_events()?;
                }
            }

            // Check if the parents's handle was dropped (such as when the host
            // requested the window to close)
            //
            // FIXME: This will need to be changed from just setting an atomic to somehow
            // synchronizing with the window being closed (using a synchronous channel, or
            // by joining on the event loop thread).
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
                if event.format == 32
                    && event.data.as_data32()[0]
                        == self.window.xcb_connection.atoms.WM_DELETE_WINDOW
                {
                    self.handle_close_requested();
                }
            }

            XEvent::ConfigureNotify(event) => {
                let new_physical_size = PhySize::new(event.width as u32, event.height as u32);

                if self.new_physical_size.is_some()
                    || new_physical_size != self.window.window_info.physical_size()
                {
                    self.new_physical_size = Some(new_physical_size);
                }
            }

            ////
            // mouse
            ////
            XEvent::MotionNotify(event) => {
                let physical_pos = PhyPoint::new(event.event_x as i32, event.event_y as i32);
                let logical_pos = physical_pos.to_logical(&self.window.window_info);

                self.handler.on_event(
                    &mut crate::Window::new(Window { inner: &self.window }),
                    Event::Mouse(MouseEvent::CursorMoved {
                        position: logical_pos,
                        modifiers: key_mods(event.state),
                    }),
                );
            }

            XEvent::EnterNotify(event) => {
                self.handler.on_event(
                    &mut crate::Window::new(Window { inner: &self.window }),
                    Event::Mouse(MouseEvent::CursorEntered),
                );
                // since no `MOTION_NOTIFY` event is generated when `ENTER_NOTIFY` is generated,
                // we generate a CursorMoved as well, so the mouse position from here isn't lost
                let physical_pos = PhyPoint::new(event.event_x as i32, event.event_y as i32);
                let logical_pos = physical_pos.to_logical(&self.window.window_info);
                self.handler.on_event(
                    &mut crate::Window::new(Window { inner: &self.window }),
                    Event::Mouse(MouseEvent::CursorMoved {
                        position: logical_pos,
                        modifiers: key_mods(event.state),
                    }),
                );
            }

            XEvent::LeaveNotify(_) => {
                self.handler.on_event(
                    &mut crate::Window::new(Window { inner: &self.window }),
                    Event::Mouse(MouseEvent::CursorLeft),
                );
            }

            XEvent::ButtonPress(event) => match event.detail {
                4..=7 => {
                    self.handler.on_event(
                        &mut crate::Window::new(Window { inner: &self.window }),
                        Event::Mouse(MouseEvent::WheelScrolled {
                            delta: match event.detail {
                                4 => ScrollDelta::Lines { x: 0.0, y: 1.0 },
                                5 => ScrollDelta::Lines { x: 0.0, y: -1.0 },
                                6 => ScrollDelta::Lines { x: -1.0, y: 0.0 },
                                7 => ScrollDelta::Lines { x: 1.0, y: 0.0 },
                                _ => unreachable!(),
                            },
                            modifiers: key_mods(event.state),
                        }),
                    );
                }
                detail => {
                    let button_id = mouse_id(detail);
                    self.handler.on_event(
                        &mut crate::Window::new(Window { inner: &self.window }),
                        Event::Mouse(MouseEvent::ButtonPressed {
                            button: button_id,
                            modifiers: key_mods(event.state),
                        }),
                    );
                }
            },

            XEvent::ButtonRelease(event) => {
                if !(4..=7).contains(&event.detail) {
                    let button_id = mouse_id(event.detail);
                    self.handler.on_event(
                        &mut crate::Window::new(Window { inner: &self.window }),
                        Event::Mouse(MouseEvent::ButtonReleased {
                            button: button_id,
                            modifiers: key_mods(event.state),
                        }),
                    );
                }
            }

            ////
            // keys
            ////
            XEvent::KeyPress(event) => {
                self.handler.on_event(
                    &mut crate::Window::new(Window { inner: &self.window }),
                    Event::Keyboard(convert_key_press_event(&event)),
                );
            }

            XEvent::KeyRelease(event) => {
                self.handler.on_event(
                    &mut crate::Window::new(Window { inner: &self.window }),
                    Event::Keyboard(convert_key_release_event(&event)),
                );
            }

            _ => {}
        }
    }

    fn handle_close_requested(&mut self) {
        // FIXME: handler should decide whether window stays open or not
        self.handle_must_close();
    }

    fn handle_must_close(&mut self) {
        self.handler.on_event(
            &mut crate::Window::new(Window { inner: &self.window }),
            Event::Window(WindowEvent::WillClose),
        );

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
