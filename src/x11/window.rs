use std::ffi::CStr;
use std::os::raw::{c_ulong, c_void};

use super::XcbConnection;
use crate::{Event, MouseButtonID, MouseScroll, Parent, WindowHandler, WindowOpenOptions};

use raw_window_handle::{unix::XlibHandle, HasRawWindowHandle, RawWindowHandle};

pub struct Window {
    xcb_connection: XcbConnection,
    window_id: u32,
    scaling: f64,
}

impl Window {
    pub fn open<H: WindowHandler>(options: WindowOpenOptions) -> WindowHandle {
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

        let scaling = get_scaling_xft(&xcb_connection)
            .or(get_scaling_screen_dimensions(&xcb_connection))
            .unwrap_or(1.0);

        let mut window = Self {
            xcb_connection,
            window_id,
            scaling,
        };

        let mut handler = H::build(&mut window);

        run_event_loop(&mut window, &mut handler);

        WindowHandle
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

pub struct WindowHandle;

// Event loop
fn run_event_loop<H: WindowHandler>(window: &mut Window, handler: &mut H) {
    loop {
        let ev = window.xcb_connection.conn.wait_for_event();
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
                    handler.draw(window);
                }
                xcb::MOTION_NOTIFY => {
                    let event = unsafe { xcb::cast_event::<xcb::MotionNotifyEvent>(&event) };
                    let detail = event.detail();

                    if detail != 4 && detail != 5 {
                        handler.on_event(
                            window,
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
                                window,
                                Event::MouseScroll(MouseScroll {
                                    x_delta: 0.0,
                                    y_delta: 1.0,
                                }),
                            );
                        }
                        5 => {
                            handler.on_event(
                                window,
                                Event::MouseScroll(MouseScroll {
                                    x_delta: 0.0,
                                    y_delta: -1.0,
                                }),
                            );
                        }
                        detail => {
                            let button_id = mouse_id(detail);
                            handler.on_event(window, Event::MouseDown(button_id));
                        }
                    }
                }
                xcb::BUTTON_RELEASE => {
                    let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                    let detail = event.detail();

                    if detail != 4 && detail != 5 {
                        let button_id = mouse_id(detail);
                        handler.on_event(window, Event::MouseUp(button_id));
                    }
                }
                xcb::KEY_PRESS => {
                    let event = unsafe { xcb::cast_event::<xcb::KeyPressEvent>(&event) };
                    let detail = event.detail();

                    handler.on_event(window, Event::KeyDown(detail));
                }
                xcb::KEY_RELEASE => {
                    let event = unsafe { xcb::cast_event::<xcb::KeyReleaseEvent>(&event) };
                    let detail = event.detail();

                    handler.on_event(window, Event::KeyUp(detail));
                }
                _ => {
                    println!("Unhandled event type: {:?}", event_type);
                }
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
