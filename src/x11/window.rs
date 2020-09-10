use std::ffi::CStr;
use std::os::raw::{c_ulong, c_void};
use std::sync::mpsc;

use super::XcbConnection;
use crate::{
    AppWindow, Event, FileDropEvent, KeyCode, KeyboardEvent, ModifiersState, MouseButton,
    MouseCursor, MouseEvent, Parent, ScrollDelta, WindowEvent, WindowInfo, WindowOpenOptions,
    WindowState,
};

use raw_window_handle::RawWindowHandle;

pub struct Window<A: AppWindow> {
    scaling: f64,
    xcb_connection: XcbConnection,
    app_window: A,
    app_message_rx: mpsc::Receiver<A::AppMessage>,
    mouse_cursor: MouseCursor,
    window_state: WindowState,
}

impl<A: AppWindow> Window<A> {
    pub fn open(options: WindowOpenOptions, app_message_rx: mpsc::Receiver<A::AppMessage>) -> Self {
        // Convert the parent to a X11 window ID if we're given one
        let parent = match options.parent {
            Parent::None => None,
            Parent::AsIfParented => None, // TODO: ???
            Parent::WithParent(p) => Some(p as u32),
        };

        // Connect to the X server
        let xcb_connection = XcbConnection::new();

        // Get screen information (?)
        let setup = xcb_connection.conn.get_setup();
        let screen = setup
            .roots()
            .nth(xcb_connection.xlib_display as usize)
            .unwrap();

        let foreground = xcb_connection.conn.generate_id();

        // Convert parent into something that X understands
        let parent_id = if let Some(p) = parent {
            p
        } else {
            screen.root()
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

        let raw_handle = RawWindowHandle::Xlib(raw_window_handle::unix::XlibHandle {
            window: window_id as c_ulong,
            display: xcb_connection.conn.get_raw_dpy() as *mut c_void,
            ..raw_window_handle::unix::XlibHandle::empty()
        });

        let scaling = get_scaling_xft(&xcb_connection)
            .or(get_scaling_screen_dimensions(&xcb_connection))
            .unwrap_or(1.0);

        let mut window_state = WindowState::new(
            options.width as u32,
            options.height as u32,
            scaling,
            raw_handle,
        );

        let app_window = A::build(&mut window_state);

        let mut x11_window = Self {
            scaling,
            xcb_connection,
            app_window,
            app_message_rx,
            mouse_cursor: Default::default(),
            window_state,
        };

        x11_window.run_event_loop();

        x11_window
    }

    // Event loop
    fn run_event_loop(&mut self) {
        loop {
            // somehow poll self.app_message_rx for messages at the same time

            let ev = self.xcb_connection.conn.wait_for_event();
            if let Some(event) = ev {
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
                        self.app_window.draw();
                    }
                    xcb::MOTION_NOTIFY => {
                        let event = unsafe { xcb::cast_event::<xcb::MotionNotifyEvent>(&event) };
                        let detail = event.detail();

                        if detail != 4 && detail != 5 {
                            self.app_window.on_event(
                                Event::Mouse(MouseEvent::CursorMoved {
                                    x: event.event_x() as f32,
                                    y: event.event_y() as f32,
                                }),
                                &mut self.window_state,
                            );
                        }
                    }
                    xcb::BUTTON_PRESS => {
                        let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                        let detail = event.detail();

                        match detail {
                            4 => {
                                self.app_window.on_event(
                                    Event::Mouse(MouseEvent::WheelScrolled {
                                        delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
                                    }),
                                    &mut self.window_state,
                                );
                            }
                            5 => {
                                self.app_window.on_event(
                                    Event::Mouse(MouseEvent::WheelScrolled {
                                        delta: ScrollDelta::Lines { x: 0.0, y: -1.0 },
                                    }),
                                    &mut self.window_state,
                                );
                            }
                            detail => {
                                let button = mouse_id(detail);
                                self.app_window.on_event(
                                    Event::Mouse(MouseEvent::ButtonPressed(button)),
                                    &mut self.window_state,
                                );
                            }
                        }
                    }
                    xcb::BUTTON_RELEASE => {
                        let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                        let detail = event.detail();

                        if detail != 4 && detail != 5 {
                            let button = mouse_id(detail);
                            self.app_window.on_event(
                                Event::Mouse(MouseEvent::ButtonReleased(button)),
                                &mut self.window_state,
                            );
                        }
                    }
                    xcb::KEY_PRESS => {
                        let event = unsafe { xcb::cast_event::<xcb::KeyPressEvent>(&event) };
                        let detail = event.detail();

                        self.app_window.on_event(
                            Event::Keyboard(KeyboardEvent::KeyPressed {
                                key_code: KeyCode::Other(detail as u32),
                                modifiers: ModifiersState::default(), // TODO: keyboard modifiers
                            }),
                            &mut self.window_state,
                        );
                    }
                    xcb::KEY_RELEASE => {
                        let event = unsafe { xcb::cast_event::<xcb::KeyReleaseEvent>(&event) };
                        let detail = event.detail();

                        self.app_window.on_event(
                            Event::Keyboard(KeyboardEvent::KeyReleased {
                                key_code: KeyCode::Other(detail as u32),
                                modifiers: ModifiersState::default(), // TODO: keyboard modifiers
                            }),
                            &mut self.window_state,
                        );
                    }
                    _ => {
                        println!("Unhandled event type: {:?}", event_type);
                    }
                }
            }

            // TODO: process requests
            if self.window_state.poll_redraw_request() {
                // do something
            }
            if let Some(mouse_cursor) = self.window_state.poll_cursor_request() {
                // do something
            }
            if self.window_state.poll_close_request() {
                // do something
            }
        }
    }
}

// Try to get the scaling with this function first.
// If this gives you `None`, fall back to `get_scaling_screen_dimensions`.
// If neither work, I guess just assume 96.0 and don't do any scaling.
fn get_scaling_xft(xcb_connection: &XcbConnection) -> Option<f64> {
    use std::ffi::CString;
    use x11::xlib::{
        XResourceManagerString, XrmDestroyDatabase, XrmGetResource, XrmGetStringDatabase, XrmValue,
    };

    let display = xcb_connection.conn.get_raw_dpy();
    unsafe {
        let rms = XResourceManagerString(display);
        if !rms.is_null() {
            let db = XrmGetStringDatabase(rms);
            if !db.is_null() {
                let mut value = XrmValue {
                    size: 0,
                    addr: std::ptr::null_mut(),
                };

                let mut value_type: *mut libc::c_char = std::ptr::null_mut();
                let name_c_str = CString::new("Xft.dpi").unwrap();
                let c_str = CString::new("Xft.Dpi").unwrap();

                let dpi = if XrmGetResource(
                    db,
                    name_c_str.as_ptr(),
                    c_str.as_ptr(),
                    &mut value_type,
                    &mut value,
                ) != 0
                    && !value.addr.is_null()
                {
                    let value_addr: &CStr = CStr::from_ptr(value.addr);
                    value_addr.to_str().ok();
                    let value_str = value_addr.to_str().ok()?;
                    let value_f64: f64 = value_str.parse().ok()?;
                    let dpi_to_scale = value_f64 / 96.0;
                    Some(dpi_to_scale)
                } else {
                    None
                };
                XrmDestroyDatabase(db);

                return dpi;
            }
        }
    }
    None
}

// Try to get the scaling with `get_scaling_xft` first.
// Only use this function as a fallback.
// If neither work, I guess just assume 96.0 and don't do any scaling.
fn get_scaling_screen_dimensions(xcb_connection: &XcbConnection) -> Option<f64> {
    // Figure out screen information
    let setup = xcb_connection.conn.get_setup();
    let screen = setup
        .roots()
        .nth(xcb_connection.xlib_display as usize)
        .unwrap();

    // Get the DPI from the screen struct
    //
    // there are 2.54 centimeters to an inch; so there are 25.4 millimeters.
    // dpi = N pixels / (M millimeters / (25.4 millimeters / 1 inch))
    //     = N pixels / (M inch / 25.4)
    //     = N * 25.4 pixels / M inch
    let width_px = screen.width_in_pixels() as f64;
    let width_mm = screen.width_in_millimeters() as f64;
    let height_px = screen.height_in_pixels() as f64;
    let height_mm = screen.height_in_millimeters() as f64;
    let _xres = width_px * 25.4 / width_mm;
    let yres = height_px * 25.4 / height_mm;

    let yscale = yres / 96.0;

    // TODO: choose between `xres` and `yres`? (probably both are the same?)
    Some(yscale)
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
