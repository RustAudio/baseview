// TODO: messy for now, will refactor when I have more of an idea of the API/architecture
// TODO: close window
// TODO: proper error handling (no bare `unwrap`s, no panics)
// TODO: move more OpenGL-related stuff into opengl_util.rs
// TODO: consider researching all unsafe calls here and figuring out what invariants need to be upheld.
//       (write safe wrappers?)

use std::ffi::CStr;
use std::os::raw::{c_int, c_void};
use std::ptr::null_mut;

use ::x11::xlib;
// use xcb::dri2; // needed later

#[cfg(all(not(feature = "wgpu_renderer"), feature = "gl_renderer"))]
use super::opengl_util;

use super::XcbConnection;
use crate::{Message, MouseButtonID, MouseScroll, Parent, Receiver, WindowOpenOptions};

pub struct Window<R: Receiver> {
    xcb_connection: XcbConnection,
    scaling: Option<f64>, // DPI scale, 96.0 is "default".
    receiver: R,

    #[cfg(all(not(feature = "wgpu_renderer"), feature = "gl_renderer"))]
    ctx: *mut x11::glx::__GLXcontextRec,
}

impl<R: Receiver> Window<R> {
    pub fn open(options: WindowOpenOptions, receiver: R) -> Self {
        // Convert the parent to a X11 window ID if we're given one
        let parent = match options.parent {
            Parent::None => None,
            Parent::AsIfParented => None, // TODO: ???
            Parent::WithParent(p) => Some(p as u32),
        };

        // Connect to the X server
        let xcb_connection = XcbConnection::new();

        #[cfg(all(feature = "gl_renderer", not(feature = "wgpu_renderer")))]
        let fb_config = opengl_util::fb_config(&xcb_connection);
        #[cfg(all(feature = "gl_renderer", not(feature = "wgpu_renderer")))]
        let x_visual_info: *const xlib::XVisualInfo =
            { opengl_util::x_visual_info(&xcb_connection, fb_config) };

        // Load up DRI2 extensions.
        // See also: https://www.x.org/releases/X11R7.7/doc/dri2proto/dri2proto.txt
        /*
        // needed later when we handle events
        let dri2_ev = {
            xcb_connection.conn.prefetch_extension_data(dri2::id());
            match xcb_connection.conn.get_extension_data(dri2::id()) {
                None => panic!("could not load dri2 extension"),
                Some(r) => r.first_event(),
            }
        };
        */

        // Get screen information (?)
        let setup = xcb_connection.conn.get_setup();
        let screen = unsafe { setup.roots().nth((*x_visual_info).screen as usize).unwrap() };

        // Convert parent into something that X understands
        let parent_id = if let Some(p) = parent {
            p
        } else {
            screen.root()
        };

        // Create a colormap
        let colormap = xcb_connection.conn.generate_id();
        unsafe {
            xcb::create_colormap(
                &xcb_connection.conn,
                xcb::COLORMAP_ALLOC_NONE as u8,
                colormap,
                parent_id,
                (*x_visual_info).visualid as u32,
            );
        }

        // Create window, connecting to the parent if we have one
        let window_id = xcb_connection.conn.generate_id();
        let cw_values = [
            (xcb::CW_BACK_PIXEL, screen.white_pixel()),
            (xcb::CW_BORDER_PIXEL, screen.black_pixel()),
            (
                xcb::CW_EVENT_MASK,
                xcb::EVENT_MASK_EXPOSURE
                    | xcb::EVENT_MASK_POINTER_MOTION
                    | xcb::EVENT_MASK_BUTTON_PRESS
                    | xcb::EVENT_MASK_BUTTON_RELEASE
                    | xcb::EVENT_MASK_KEY_PRESS
                    | xcb::EVENT_MASK_KEY_RELEASE,
            ),
            (xcb::CW_COLORMAP, colormap),
        ];
        xcb::create_window(
            // Connection
            &xcb_connection.conn,
            // Depth
            unsafe { *x_visual_info }.depth as u8,
            // Window ID
            window_id,
            // Parent ID
            parent_id,
            // x
            0,
            // y
            0,
            // width
            options.width as u16,
            // height
            options.height as u16,
            // border width
            0,
            // class
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            // visual
            unsafe { *x_visual_info }.visualid as u32,
            // value list
            &cw_values,
        );

        // Don't need the visual info anymore
        unsafe {
            xlib::XFree(x_visual_info as *mut c_void);
        }

        // Change window title
        let title = options.title;
        xcb::change_property(
            &xcb_connection.conn,
            xcb::PROP_MODE_REPLACE as u8,
            window_id,
            xcb::ATOM_WM_NAME,
            xcb::ATOM_STRING,
            8,
            title.as_bytes(),
        );

        // TODO: This requires a global, which is a no. Figure out if there's a better way to do it.
        /*
        // installing an event handler to check if error is generated
        unsafe {
            ctx_error_occurred = false;
        }
        let old_handler = unsafe {
            xlib::XSetErrorHandler(Some(ctx_error_handler))
        };
        */

        #[cfg(all(feature = "gl_renderer", not(feature = "wgpu_renderer")))]
        let ctx = opengl_util::glx_context(&xcb_connection, fb_config);

        // Display the window
        xcb::map_window(&xcb_connection.conn, window_id);
        xcb_connection.conn.flush();
        unsafe {
            xlib::XSync(xcb_connection.conn.get_raw_dpy(), xlib::False);
        }

        #[cfg(all(feature = "gl_renderer", not(feature = "wgpu_renderer")))]
        let mut x11_window = Self {
            xcb_connection,
            scaling: None,
            ctx,
            receiver,
        };

        x11_window.scaling = x11_window
            .get_scaling_xft()
            .or(x11_window.get_scaling_screen_dimensions());
        println!("Scale factor: {:?}", x11_window.scaling);

        x11_window
            .receiver
            .on_message(Message::Opened(crate::message::WindowInfo {
                width: options.width as u32,
                height: options.height as u32,
                dpi: x11_window.scaling,
            }));

        x11_window.handle_events(window_id);

        x11_window.receiver.on_message(Message::WillClose);

        return x11_window;
    }

    // Event loop
    fn handle_events(&mut self, window_id: u32) {
        let raw_display = self.xcb_connection.conn.get_raw_dpy();
        loop {
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
                        #[cfg(all(feature = "gl_renderer", not(feature = "wgpu_renderer")))]
                        opengl_util::xcb_expose(window_id, raw_display, self.ctx);
                    }
                    xcb::MOTION_NOTIFY => {
                        let event = unsafe { xcb::cast_event::<xcb::MotionNotifyEvent>(&event) };
                        let detail = event.detail();

                        if detail != 4 && detail != 5 {
                            self.receiver.on_message(Message::CursorMotion(
                                event.event_x() as i32,
                                event.event_y() as i32,
                            ));
                        }
                    }
                    xcb::BUTTON_PRESS => {
                        let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                        let detail = event.detail();

                        match detail {
                            4 => {
                                self.receiver.on_message(Message::MouseScroll(MouseScroll {
                                    x_delta: 0.0,
                                    y_delta: 1.0,
                                }));
                            }
                            5 => {
                                self.receiver.on_message(Message::MouseScroll(MouseScroll {
                                    x_delta: 0.0,
                                    y_delta: -1.0,
                                }));
                            }
                            detail => {
                                let button_id = mouse_id(detail);
                                self.receiver.on_message(Message::MouseDown(button_id));
                            }
                        }
                    }
                    xcb::BUTTON_RELEASE => {
                        let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                        let detail = event.detail();

                        if detail != 4 && detail != 5 {
                            let button_id = mouse_id(detail);
                            self.receiver.on_message(Message::MouseUp(button_id));
                        }
                    }
                    xcb::KEY_PRESS => {
                        let event = unsafe { xcb::cast_event::<xcb::KeyPressEvent>(&event) };
                        let detail = event.detail();

                        self.receiver.on_message(Message::KeyDown(detail));
                    }
                    xcb::KEY_RELEASE => {
                        let event = unsafe { xcb::cast_event::<xcb::KeyReleaseEvent>(&event) };
                        let detail = event.detail();

                        self.receiver.on_message(Message::KeyUp(detail));
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
    fn get_scaling_xft(&self) -> Option<f64> {
        use std::ffi::CString;
        use x11::xlib::{
            XResourceManagerString, XrmDestroyDatabase, XrmGetResource, XrmGetStringDatabase,
            XrmValue,
        };

        let display = self.xcb_connection.conn.get_raw_dpy();
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
                        let value_f64 = value_str.parse().ok()?;
                        Some(value_f64)
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
    fn get_scaling_screen_dimensions(&self) -> Option<f64> {
        // Figure out screen information
        let setup = self.xcb_connection.conn.get_setup();
        let screen = setup
            .roots()
            .nth(self.xcb_connection.xlib_display as usize)
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

        // TODO: choose between `xres` and `yres`? (probably both are the same?)
        Some(yres)
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
