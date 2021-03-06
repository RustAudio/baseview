use std::ffi::CString;
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
    Event, MouseButton, MouseCursor, MouseEvent, ScrollDelta,
    WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions,
    WindowScalePolicy, PhyPoint, PhySize,
};

use super::keyboard::{convert_key_press_event, convert_key_release_event};

pub struct Window {
    xcb_connection: XcbConnection,
    window_id: u32,
    window_info: WindowInfo,
    mouse_cursor: MouseCursor,

    frame_interval: Duration,
    event_loop_running: bool,

    new_physical_size: Option<PhySize>,
}

// Hack to allow sending a RawWindowHandle between threads. Do not make public
struct SendableRwh(RawWindowHandle);

unsafe impl Send for SendableRwh {}

type WindowOpenResult = Result<SendableRwh, ()>;

impl Window {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B)
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        // Convert parent into something that X understands
        let parent_id = match parent.raw_window_handle() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window,
            h => panic!("unsupported parent handle type {:?}", h),
        };

        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let thread = thread::spawn(move || {
            Self::window_thread(Some(parent_id), options, build, tx.clone());
        });

        let _ = rx.recv().unwrap().unwrap();
    }

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> RawWindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let thread = thread::spawn(move || {
            Self::window_thread(None, options, build, tx.clone());
        });

        rx.recv().unwrap().unwrap().0
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let thread = thread::spawn(move || {
            Self::window_thread(None, options, build, tx.clone());
        });

        let _ = rx.recv().unwrap().unwrap();

        thread.join();
    }

    fn window_thread<H, B>(
        parent: Option<u32>,
        options: WindowOpenOptions, build: B,
        tx: mpsc::SyncSender<WindowOpenResult>,
    )
    where H: WindowHandler + 'static,
          B: FnOnce(&mut crate::Window) -> H,
          B: Send + 'static,
    {
        // Connect to the X server
        let xcb_connection = XcbConnection::new();

        let foreground = unsafe { xcb_sys::xcb_generate_id(xcb_connection.conn) };
        let parent_id = parent.unwrap_or_else(|| unsafe { (*xcb_connection.screen).root });
        let value_mask = xcb_sys::XCB_GC_FOREGROUND | xcb_sys::XCB_GC_GRAPHICS_EXPOSURES;
        let value_list = &[
            unsafe { (*xcb_connection.screen).black_pixel },
            0,
        ];
        unsafe {
            xcb_sys::xcb_create_gc(
                xcb_connection.conn,
                foreground,
                parent_id,
                value_mask,
                value_list.as_ptr() as *const c_void,
            );
        }

        let scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => xcb_connection.get_scaling().unwrap_or(1.0),
            WindowScalePolicy::ScaleFactor(scale) => scale
        };

        let window_info = WindowInfo::from_logical_size(options.size, scaling);

        let window_id = unsafe { xcb_sys::xcb_generate_id(xcb_connection.conn) };
        let value_mask = xcb_sys::XCB_CW_EVENT_MASK;
        let value_list = &[
            xcb_sys::XCB_EVENT_MASK_EXPOSURE
                | xcb_sys::XCB_EVENT_MASK_POINTER_MOTION
                | xcb_sys::XCB_EVENT_MASK_BUTTON_PRESS
                | xcb_sys::XCB_EVENT_MASK_BUTTON_RELEASE
                | xcb_sys::XCB_EVENT_MASK_KEY_PRESS
                | xcb_sys::XCB_EVENT_MASK_KEY_RELEASE
                | xcb_sys::XCB_EVENT_MASK_STRUCTURE_NOTIFY,
        ];
        unsafe {
            xcb_sys::xcb_create_window(
                xcb_connection.conn,
                xcb_sys::XCB_COPY_FROM_PARENT as u8,
                window_id,
                parent_id,
                0,                     // x coordinate of the new window
                0,                     // y coordinate of the new window
                window_info.physical_size().width as u16,        // window width
                window_info.physical_size().height as u16,       // window height
                0,                     // window border
                xcb_sys::XCB_WINDOW_CLASS_INPUT_OUTPUT as u16,
                (*xcb_connection.screen).root_visual,
                value_mask,
                value_list.as_ptr() as *const c_void,
            );
        }

        unsafe {
            xcb_sys::xcb_map_window(xcb_connection.conn, window_id);
        }

        // Change window title
        let title = CString::new(options.title).unwrap();
        unsafe {
            xcb_sys::xcb_change_property(
                xcb_connection.conn,
                xcb_sys::XCB_PROP_MODE_REPLACE as u8,
                window_id,
                xcb_sys::XCB_ATOM_WM_NAME,
                xcb_sys::XCB_ATOM_STRING,
                8, // view data as 8-bit
                title.as_bytes().len() as u32,
                title.as_ptr() as *const c_void,
            );
        }

        let atoms = &[xcb_connection.atoms.wm_delete_window];
        unsafe {
            xcb_sys::xcb_icccm_set_wm_protocols(
                xcb_connection.conn,
                window_id,
                xcb_connection.atoms.wm_protocols,
                atoms.len() as u32,
                atoms.as_ptr() as *mut xcb_sys::xcb_atom_t,
            );
        }

        unsafe {
            xcb_sys::xcb_flush(xcb_connection.conn);
        }

        let mut window = Self {
            xcb_connection,
            window_id,
            window_info,
            mouse_cursor: MouseCursor::default(),

            frame_interval: Duration::from_millis(15),
            event_loop_running: false,

            new_physical_size: None,
        };

        let mut handler = build(&mut crate::Window::new(&mut window));

        let _ = tx.send(Ok(SendableRwh(window.raw_window_handle())));

        window.run_event_loop(&mut handler);
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
            let value_mask = xcb_sys::XCB_CW_CURSOR;
            let value_list = &[xid];
            unsafe {
                xcb_sys::xcb_change_window_attributes(
                    self.xcb_connection.conn,
                    self.window_id,
                    value_mask,
                    value_list.as_ptr() as *const c_void,
                );

                xcb_sys::xcb_flush(self.xcb_connection.conn);
            }
        }

        self.mouse_cursor = mouse_cursor;
    }

    #[inline]
    fn drain_xcb_events(&mut self, handler: &mut dyn WindowHandler) {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_physical_size = None;

        loop {
            unsafe {
                let event = xcb_sys::xcb_poll_for_event(self.xcb_connection.conn);
                if event.is_null() {
                    break;
                }
                self.handle_xcb_event(handler, event);
                libc::free(event as *mut c_void);
            }
        }

        if let Some(size) = self.new_physical_size.take() {
            self.window_info = WindowInfo::from_physical_size(
                size,
                self.window_info.scale()
            );

            let window_info = self.window_info;

            handler.on_event(
                &mut crate::Window::new(self),
                Event::Window(WindowEvent::Resized(window_info))
            );
        }
    }

    // Event loop
    // FIXME: poll() acts fine on linux, sometimes funky on *BSD. XCB upstream uses a define to
    // switch between poll() and select() (the latter of which is fine on *BSD), and we should do
    // the same.
    fn run_event_loop(&mut self, handler: &mut dyn WindowHandler) {
        use nix::poll::*;

        let xcb_fd = unsafe {
            xcb_sys::xcb_get_file_descriptor(self.xcb_connection.conn)
        };

        let mut next_frame = Instant::now() + self.frame_interval;
        self.event_loop_running = true;

        while self.event_loop_running {
            let now = Instant::now();
            let until_next_frame = if now > next_frame {
                handler.on_frame(&mut crate::Window::new(self));

                next_frame = Instant::now() + self.frame_interval;
                self.frame_interval
            } else {
                next_frame - now
            };

            let mut fds = [PollFd::new(xcb_fd, PollFlags::POLLIN)];

            // Check for any events in the internal buffers
            // before going to sleep:
            self.drain_xcb_events(handler);

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

    fn handle_xcb_event(&mut self, handler: &mut dyn WindowHandler, event: *mut xcb_sys::xcb_generic_event_t) {
        let event_type = (unsafe { (*event).response_type } & !0x80) as u32;

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
            xcb_sys::XCB_CLIENT_MESSAGE => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_client_message_event_t) };

                if unsafe { event.data.data32[0] } == self.xcb_connection.atoms.wm_delete_window {
                    handler.on_event(
                        &mut crate::Window::new(self),
                        Event::Window(WindowEvent::WillClose)
                    );

                    // FIXME: handler should decide whether window stays open or not
                    self.event_loop_running = false;
                }
            }

            xcb_sys::XCB_CONFIGURE_NOTIFY => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_configure_notify_event_t) };

                let new_physical_size = PhySize::new(event.width as u32, event.height as u32);

                if self.new_physical_size.is_some() || new_physical_size != self.window_info.physical_size() {
                    self.new_physical_size = Some(new_physical_size);
                }
            }

            ////
            // mouse
            ////
            xcb_sys::XCB_MOTION_NOTIFY => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_motion_notify_event_t) };

                if event.detail != 4 && event.detail != 5 {
                    let physical_pos = PhyPoint::new(event.event_x as i32, event.event_y as i32);
                    let logical_pos = physical_pos.to_logical(&self.window_info);

                    handler.on_event(
                        &mut crate::Window::new(self),
                        Event::Mouse(MouseEvent::CursorMoved {
                            position: logical_pos,
                        }),
                    );
                }
            }

            xcb_sys::XCB_BUTTON_PRESS => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_button_press_event_t) };

                match event.detail {
                    4 => {
                        handler.on_event(
                            &mut crate::Window::new(self),
                            Event::Mouse(MouseEvent::WheelScrolled(ScrollDelta::Lines {
                                x: 0.0,
                                y: 1.0,
                            })),
                        );
                    }
                    5 => {
                        handler.on_event(
                            &mut crate::Window::new(self),
                            Event::Mouse(MouseEvent::WheelScrolled(ScrollDelta::Lines {
                                x: 0.0,
                                y: -1.0,
                            })),
                        );
                    }
                    detail => {
                        let button_id = mouse_id(detail);
                        handler.on_event(
                            &mut crate::Window::new(self),
                            Event::Mouse(MouseEvent::ButtonPressed(button_id))
                        );
                    }
                }
            }

            xcb_sys::XCB_BUTTON_RELEASE => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_button_press_event_t) };

                if event.detail != 4 && event.detail != 5 {
                    let button_id = mouse_id(event.detail);
                    handler.on_event(
                        &mut crate::Window::new(self),
                        Event::Mouse(MouseEvent::ButtonReleased(button_id))
                    );
                }
            }

            ////
            // keys
            ////
            xcb_sys::XCB_KEY_PRESS => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_key_press_event_t) };

                handler.on_event(
                    &mut crate::Window::new(self),
                    Event::Keyboard(convert_key_press_event(event))
                );
            }

            xcb_sys::XCB_KEY_RELEASE => {
                let event = unsafe { &*(event as *mut xcb_sys::xcb_key_release_event_t) };

                handler.on_event(
                    &mut crate::Window::new(self),
                    Event::Keyboard(convert_key_release_event(event))
                );
            }

            _ => {}
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Xlib(XlibHandle {
            window: self.window_id as c_ulong,
            display: self.xcb_connection.display as *mut c_void,
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
