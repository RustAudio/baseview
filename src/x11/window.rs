use std::os::raw::{c_ulong, c_void};
use std::sync::mpsc;
use std::time::*;
use std::thread;

use raw_window_handle::{
    unix::XlibHandle,
    HasRawWindowHandle,
    RawWindowHandle
};

use super::XcbConnection;
use crate::{
    Event, KeyboardEvent, MouseButton, MouseCursor, MouseEvent, Parent, ScrollDelta, WindowEvent,
    WindowHandler,WindowInfo, WindowOpenOptions,
};

pub struct Window {
    xcb_connection: XcbConnection,
    window_id: u32,
    window_info: WindowInfo,
    mouse_cursor: MouseCursor,

    frame_interval: Duration,
    event_loop_running: bool,

    new_size: Option<(u32, u32)>
}

// FIXME: move to outer crate context
pub struct WindowHandle {
    thread: std::thread::JoinHandle<()>,
}

impl WindowHandle {
    pub fn app_run_blocking(self) {
        let _ = self.thread.join();
    }
}

type WindowOpenResult = Result<(), ()>;

impl Window {
    pub fn open<H: WindowHandler>(options: WindowOpenOptions) -> WindowHandle {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let thread = thread::spawn(move || {
            if let Err(e) = Self::window_thread::<H>(options, tx.clone()) {
                let _ = tx.send(Err(e));
            }
        });

        // FIXME: placeholder types for returning errors in the future
        let _ = rx.recv();

        WindowHandle { thread }
    }

    fn window_thread<H: WindowHandler>(
        options: WindowOpenOptions, tx: mpsc::SyncSender<WindowOpenResult>,
    ) -> WindowOpenResult {
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
            Parent::WithParent(RawWindowHandle::Xlib(h)) => h.window as u32,
            Parent::WithParent(RawWindowHandle::Xcb(h)) => h.window,
            Parent::WithParent(h) =>
                panic!("unsupported parent handle type {:?}", h),

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
                    | xcb::EVENT_MASK_KEY_RELEASE
                    | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
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

        xcb_connection.atoms.wm_protocols
            .zip(xcb_connection.atoms.wm_delete_window)
            .map(|(wm_protocols, wm_delete_window)| {
                xcb_util::icccm::set_wm_protocols(
                    &xcb_connection.conn,
                    window_id,
                    wm_protocols,
                    &[wm_delete_window],
                );
            });

        xcb_connection.conn.flush();

        let scaling = xcb_connection.get_scaling().unwrap_or(1.0);

        let window_info = WindowInfo {
            width: options.width as u32,
            height: options.height as u32,
            scale: scaling,
        };

        let mut window = Self {
            xcb_connection,
            window_id,
            window_info,
            mouse_cursor: MouseCursor::default(),

            frame_interval: Duration::from_secs_f64(options.frame_interval_secs),
            event_loop_running: false,

            new_size: None
        };

        let mut handler = H::build(&mut window);

        let _ = tx.send(Ok(()));

        window.run_event_loop(&mut handler);
        Ok(())
    }

    pub fn window_info(&self) -> &WindowInfo {
        &self.window_info
    }

    pub fn set_mouse_cursor(&mut self, mouse_cursor: MouseCursor) {
        if self.mouse_cursor == mouse_cursor {
            return
        }

        let xid = self.xcb_connection.get_cursor_xid(mouse_cursor);

        if xid != 0 {
            xcb::change_window_attributes(
                &self.xcb_connection.conn,
                self.window_id,
                &[(xcb::CW_CURSOR, xid)]
            );

            self.xcb_connection.conn.flush();
        }

        self.mouse_cursor = mouse_cursor;
    }

    #[inline]
    fn drain_xcb_events<H: WindowHandler>(&mut self, handler: &mut H) {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_size = None;

        while let Some(event) = self.xcb_connection.conn.poll_for_event() {
            self.handle_xcb_event(handler, event);
        }

        if let Some((width, height)) = self.new_size.take() {
            self.window_info.width = width;
            self.window_info.height = height;

            handler.on_event(self, Event::Window(
                WindowEvent::Resized(self.window_info)
            ))
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
            let until_next_frame = if now > next_frame {
                handler.on_frame();

                next_frame = now + self.frame_interval;
                self.frame_interval
            } else {
                next_frame - now
            };

            let mut fds = [PollFd::new(xcb_fd, PollFlags::POLLIN)];

            // FIXME: handle errors
            poll(&mut fds, until_next_frame.subsec_millis() as i32).unwrap();

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
            ////
            // window
            ////
            xcb::EXPOSE => {
                handler.on_frame();
            }

            xcb::CLIENT_MESSAGE => {
                let event = unsafe { xcb::cast_event::<xcb::ClientMessageEvent>(&event) };

                // what an absolute tragedy this all is
                let data = event.data().data;
                let (_, data32, _) = unsafe { data.align_to::<u32>() };

                let wm_delete_window = self.xcb_connection.atoms.wm_delete_window
                    .unwrap_or(xcb::NONE);

                if wm_delete_window == data32[0] {
                    handler.on_event(self, Event::Window(WindowEvent::WillClose));

                    // FIXME: handler should decide whether window stays open or not
                    self.event_loop_running = false;
                }
            }

            xcb::CONFIGURE_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::ConfigureNotifyEvent>(&event) };

                let new_size = (event.width() as u32, event.height() as u32);
                let cur_size = (self.window_info.width, self.window_info.height);

                if self.new_size.is_some() || new_size != cur_size {
                    self.new_size = Some(new_size);
                }
            }

            ////
            // mouse
            ////
            xcb::MOTION_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::MotionNotifyEvent>(&event) };
                let detail = event.detail();

                if detail != 4 && detail != 5 {
                    handler.on_event(
                        self,
                        Event::Mouse(MouseEvent::CursorMoved {
                            x: event.event_x() as i32,
                            y: event.event_y() as i32,
                        }),
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
                            Event::Mouse(MouseEvent::WheelScrolled(ScrollDelta::Lines {
                                x: 0.0,
                                y: 1.0,
                            })),
                        );
                    }
                    5 => {
                        handler.on_event(
                            self,
                            Event::Mouse(MouseEvent::WheelScrolled(ScrollDelta::Lines {
                                x: 0.0,
                                y: -1.0,
                            })),
                        );
                    }
                    detail => {
                        let button_id = mouse_id(detail);
                        handler.on_event(self, Event::Mouse(MouseEvent::ButtonPressed(button_id)));
                    }
                }
            }

            xcb::BUTTON_RELEASE => {
                let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                let detail = event.detail();

                if detail != 4 && detail != 5 {
                    let button_id = mouse_id(detail);
                    handler.on_event(self, Event::Mouse(MouseEvent::ButtonReleased(button_id)));
                }
            }

            ////
            // keys
            ////
            xcb::KEY_PRESS => {
                let event = unsafe { xcb::cast_event::<xcb::KeyPressEvent>(&event) };
                let detail = event.detail();

                handler.on_event(
                    self,
                    Event::Keyboard(KeyboardEvent::KeyPressed(detail as u32)),
                );
            }

            xcb::KEY_RELEASE => {
                let event = unsafe { xcb::cast_event::<xcb::KeyReleaseEvent>(&event) };
                let detail = event.detail();

                handler.on_event(
                    self,
                    Event::Keyboard(KeyboardEvent::KeyReleased(detail as u32)),
                );
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

fn mouse_id(id: u8) -> MouseButton {
    match id {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        6 => MouseButton::Back,
        7 => MouseButton::Forward,
        id => MouseButton::Other(id),
    }
}
