use std::os::raw::{c_ulong, c_void};
use std::time::*;

use raw_window_handle::{unix::XlibHandle, HasRawWindowHandle, RawWindowHandle};

use super::XcbConnection;
use crate::{Event, MouseButtonID, MouseScroll, Parent, WindowHandler, WindowOpenOptions};


pub struct Window {
    xcb_connection: XcbConnection,
    window_id: u32,
    scaling: f64,

    frame_interval: Duration,
    event_loop_running: bool
}

// FIXME: move to outer crate context
pub struct WindowHandle;


impl Window {
    pub fn open<H: WindowHandler>(options: WindowOpenOptions) -> WindowHandle {
        // Connect to the X server
        // FIXME: baseview error type instead of unwrap()
        let xcb_connection = XcbConnection::new().unwrap();

        // Get screen information (?)
        let setup = xcb_connection.conn.get_setup();
        let screen = setup
            .roots()
            .nth(xcb_connection.xlib_display as usize)
            .unwrap();

        let foreground = xcb_connection.conn.generate_id();

        // Convert parent into something that X understands
        let parent_id = match options.parent {
            Parent::WithParent(p) => p as u32,
            Parent::None | Parent::AsIfParented => screen.root(),
        };

        xcb::create_gc(
            &xcb_connection.conn,
            foreground,
            parent_id,
            &[
                (xcb::GC_FOREGROUND, screen.black_pixel()),
                (xcb::GC_GRAPHICS_EXPOSURES, 0),
            ],
        );

        let window_id = xcb_connection.conn.generate_id();
        xcb::create_window(
            &xcb_connection.conn,
            xcb::COPY_FROM_PARENT as u8,
            window_id,
            parent_id,
            0,                     // x coordinate of the new window
            0,                     // y coordinate of the new window
            options.width as u16,  // window width
            options.height as u16, // window height
            0,                     // window border
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &[(
                xcb::CW_EVENT_MASK,
                xcb::EVENT_MASK_EXPOSURE
                    | xcb::EVENT_MASK_POINTER_MOTION
                    | xcb::EVENT_MASK_BUTTON_PRESS
                    | xcb::EVENT_MASK_BUTTON_RELEASE
                    | xcb::EVENT_MASK_KEY_PRESS
                    | xcb::EVENT_MASK_KEY_RELEASE,
            )],
        );

        xcb::map_window(&xcb_connection.conn, window_id);

        // Change window title
        let title = options.title;
        xcb::change_property(
            &xcb_connection.conn,
            xcb::PROP_MODE_REPLACE as u8,
            window_id,
            xcb::ATOM_WM_NAME,
            xcb::ATOM_STRING,
            8, // view data as 8-bit
            title.as_bytes(),
        );

        xcb_connection.conn.flush();

        let scaling = xcb_connection.get_scaling().unwrap_or(1.0);

        let mut window = Self {
            xcb_connection,
            window_id,
            scaling,

            frame_interval: Duration::from_millis(15),
            event_loop_running: false
        };

        let mut handler = H::build(&mut window);

        window.run_event_loop(&mut handler);

        WindowHandle
    }

    #[inline]
    fn drain_xcb_events<H: WindowHandler>(&mut self, handler: &mut H) {
        while let Some(event) = self.xcb_connection.conn.poll_for_event() {
            self.handle_xcb_event(handler, event);
        }
    }

    // Event loop
    // FIXME: poll() acts fine on linux, sometimes funky on *BSD. XCB upstream uses a define to
    // switch between poll() and select() (the latter of which is fine on *BSD), and we should do
    // the same.
    fn run_event_loop<H: WindowHandler>(&mut self, handler: &mut H) {
        use nix::poll::*;

        let xcb_fd = unsafe {
            let raw_conn = self.xcb_connection.conn.get_raw_conn();
            xcb::ffi::xcb_get_file_descriptor(raw_conn)
        };

        let mut next_frame = Instant::now() + self.frame_interval;
        self.event_loop_running = true;

        while self.event_loop_running {
            let now = Instant::now();
            let until_next_frame =
                if now > next_frame {
                    handler.on_frame();

                    next_frame = now + self.frame_interval;
                    self.frame_interval
                } else {
                    next_frame - now
                };

            let mut fds = [
                PollFd::new(xcb_fd, PollFlags::POLLIN)
            ];

            // FIXME: handle errors
            poll(&mut fds, until_next_frame.subsec_millis() as i32)
                .unwrap();

            if let Some(revents) = fds[0].revents() {
                if revents.contains(PollFlags::POLLERR) {
                    panic!("xcb connection poll error");
                }

                if revents.contains(PollFlags::POLLIN) {
                    self.drain_xcb_events(handler);
                }
            }
        }
    }

    fn handle_xcb_event<H: WindowHandler>(&mut self, handler: &mut H, event: xcb::GenericEvent) {
        let event_type = event.response_type() & !0x80;

        // For all of the keyboard and mouse events, you can fetch
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

        match event_type {
            xcb::EXPOSE => {
                handler.on_frame();
            }
            xcb::MOTION_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::MotionNotifyEvent>(&event) };
                let detail = event.detail();

                if detail != 4 && detail != 5 {
                    handler.on_event(
                        self,
                        Event::CursorMotion(event.event_x() as i32, event.event_y() as i32),
                    );
                }
            }
            xcb::BUTTON_PRESS => {
                let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                let detail = event.detail();

                match detail {
                    4 => {
                        handler.on_event(
                            self,
                            Event::MouseScroll(MouseScroll {
                                x_delta: 0.0,
                                y_delta: 1.0,
                            }),
                        );
                    }
                    5 => {
                        handler.on_event(
                            self,
                            Event::MouseScroll(MouseScroll {
                                x_delta: 0.0,
                                y_delta: -1.0,
                            }),
                        );
                    }
                    detail => {
                        let button_id = mouse_id(detail);
                        handler.on_event(self, Event::MouseDown(button_id));
                    }
                }
            }
            xcb::BUTTON_RELEASE => {
                let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                let detail = event.detail();

                if detail != 4 && detail != 5 {
                    let button_id = mouse_id(detail);
                    handler.on_event(self, Event::MouseUp(button_id));
                }
            }
            xcb::KEY_PRESS => {
                let event = unsafe { xcb::cast_event::<xcb::KeyPressEvent>(&event) };
                let detail = event.detail();

                handler.on_event(self, Event::KeyDown(detail));
            }
            xcb::KEY_RELEASE => {
                let event = unsafe { xcb::cast_event::<xcb::KeyReleaseEvent>(&event) };
                let detail = event.detail();

                handler.on_event(self, Event::KeyUp(detail));
            }
            _ => {
                println!("Unhandled event type: {:?}", event_type);
            }
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Xlib(XlibHandle {
            window: self.window_id as c_ulong,
            display: self.xcb_connection.conn.get_raw_dpy() as *mut c_void,
            ..raw_window_handle::unix::XlibHandle::empty()
        })
    }
}

fn mouse_id(id: u8) -> MouseButtonID {
    match id {
        1 => MouseButtonID::Left,
        2 => MouseButtonID::Middle,
        3 => MouseButtonID::Right,
        6 => MouseButtonID::Back,
        7 => MouseButtonID::Forward,
        id => MouseButtonID::Other(id),
    }
}
