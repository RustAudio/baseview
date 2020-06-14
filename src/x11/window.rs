// TODO: messy for now, will refactor when I have more of an idea of the API/architecture
// TODO: close window
// TODO: proper error handling (no bare `unwrap`s, no panics)
// TODO: move more OpenGL-related stuff into opengl_util.rs
// TODO: consider researching all unsafe calls here and figuring out what invariants need to be upheld.
//       (write safe wrappers?)

use std::ffi::CStr;
use std::os::raw::{c_int, c_void};
use std::ptr::null_mut;

use ::x11::{glx, xlib};
// use xcb::dri2; // needed later

use super::opengl_util;
use super::XcbConnection;
use crate::Parent;
use crate::WindowOpenOptions;

pub struct Window {
    xcb_connection: XcbConnection,
    scaling: Option<f64>, // DPI scale, 96.0 is "default".
}

impl Window {
    pub fn open(options: WindowOpenOptions) -> Self {
        // Convert the parent to a X11 window ID if we're given one
        let parent = match options.parent {
            Parent::None => None,
            Parent::AsIfParented => None, // TODO: ???
            Parent::WithParent(p) => Some(p as u32),
        };

        // Connect to the X server
        let xcb_connection = XcbConnection::new();

        // Check GLX version (>= 1.3 needed)
        opengl_util::check_glx_version(&xcb_connection);

        // Get GLX framebuffer config (requires GLX >= 1.3)
        #[rustfmt::skip]
        let fb_config = opengl_util::get_glxfbconfig(
            &xcb_connection,
            &[
                glx::GLX_X_RENDERABLE,  1,
                glx::GLX_DRAWABLE_TYPE, glx::GLX_WINDOW_BIT,
                glx::GLX_RENDER_TYPE,   glx::GLX_RGBA_BIT,
                glx::GLX_X_VISUAL_TYPE, glx::GLX_TRUE_COLOR,
                glx::GLX_RED_SIZE,      8,
                glx::GLX_GREEN_SIZE,    8,
                glx::GLX_BLUE_SIZE,     8,
                glx::GLX_ALPHA_SIZE,    8,
                glx::GLX_DEPTH_SIZE,    24,
                glx::GLX_STENCIL_SIZE,  8,
                glx::GLX_DOUBLEBUFFER,  1,
                0
            ],
        );

        // The GLX framebuffer config holds an XVisualInfo, which we'll need for other X operations.
        let x_visual_info: *const xlib::XVisualInfo =
            unsafe { glx::glXGetVisualFromFBConfig(xcb_connection.conn.get_raw_dpy(), fb_config) };

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
                    | xcb::EVENT_MASK_BUTTON_PRESS
                    | xcb::EVENT_MASK_BUTTON_RELEASE
                    | xcb::EVENT_MASK_BUTTON_1_MOTION,
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

        // Load GLX extensions
        // We need at least `GLX_ARB_create_context`
        let glx_extensions = unsafe {
            CStr::from_ptr(glx::glXQueryExtensionsString(
                xcb_connection.conn.get_raw_dpy(),
                xcb_connection.xlib_display,
            ))
            .to_str()
            .unwrap()
        };
        glx_extensions
            .find("GLX_ARB_create_context")
            .expect("could not find GLX extension GLX_ARB_create_context");

        // With GLX, we don't need a context pre-created in order to load symbols.
        // Otherwise, we would need to create a temporary legacy (dummy) GL context to load them.
        // (something that has at least GlXCreateContextAttribsARB)
        let glx_create_context_attribs: opengl_util::GlXCreateContextAttribsARBProc =
            unsafe { std::mem::transmute(opengl_util::load_gl_func("glXCreateContextAttribsARB")) };

        // Load all other symbols
        unsafe {
            gl::load_with(|n| opengl_util::load_gl_func(&n));
        }

        // Check GL3 support
        if !gl::GenVertexArrays::is_loaded() {
            panic!("no GL3 support available!");
        }

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

        // Create GLX context attributes. (?)
        let context_attribs: [c_int; 5] = [
            glx::arb::GLX_CONTEXT_MAJOR_VERSION_ARB as c_int,
            3,
            glx::arb::GLX_CONTEXT_MINOR_VERSION_ARB as c_int,
            0,
            0,
        ];
        let ctx = unsafe {
            glx_create_context_attribs(
                xcb_connection.conn.get_raw_dpy(),
                fb_config,
                null_mut(),
                xlib::True,
                &context_attribs[0] as *const c_int,
            )
        };

        if ctx.is_null()
        /* || ctx_error_occurred */
        {
            panic!("Error when creating a GL 3.0 context");
        }
        if unsafe { glx::glXIsDirect(xcb_connection.conn.get_raw_dpy(), ctx) } == 0 {
            panic!("Obtained indirect rendering context");
        }

        // Display the window
        xcb::map_window(&xcb_connection.conn, window_id);
        xcb_connection.conn.flush();
        unsafe {
            xlib::XSync(xcb_connection.conn.get_raw_dpy(), xlib::False);
        }

        let mut x11_window = Self {
            xcb_connection,
            scaling: None,
        };

        x11_window.scaling = x11_window
            .get_scaling_xft()
            .or(x11_window.get_scaling_screen_dimensions());
        println!("Scale factor: {:?}", x11_window.scaling);
        x11_window.handle_events(window_id, ctx);

        return x11_window;
    }

    // Event loop
    fn handle_events(&self, window_id: u32, ctx: *mut x11::glx::__GLXcontextRec) {
        let raw_display = self.xcb_connection.conn.get_raw_dpy();
        loop {
            let ev = self.xcb_connection.conn.wait_for_event();
            if let Some(event) = ev {
                let event_type = event.response_type() & !0x80;
                //println!("{:?}", event_type);

                match event_type {
                    xcb::EXPOSE => unsafe {
                        glx::glXMakeCurrent(raw_display, window_id as xlib::XID, ctx);
                        gl::ClearColor(0.3, 0.8, 0.3, 1.0);
                        gl::Clear(gl::COLOR_BUFFER_BIT);
                        gl::Flush();
                        glx::glXSwapBuffers(raw_display, window_id as xlib::XID);
                        glx::glXMakeCurrent(raw_display, 0, null_mut());
                    },
                    _ => {}
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