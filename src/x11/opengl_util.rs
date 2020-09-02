use std::ffi::{CString, CStr};
use std::os::raw::{c_int, c_void};
use std::ptr::null_mut;

use ::x11::{glx, xlib};

use super::XcbConnection;

pub fn fb_config(xcb_connection: &XcbConnection) -> *mut glx::__GLXFBConfigRec {
    // Check GLX version (>= 1.3 needed)
    check_glx_version(&xcb_connection);

    // Get GLX framebuffer config (requires GLX >= 1.3)
    #[rustfmt::skip]
    let fb_config = get_glxfbconfig(
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

    fb_config
}

pub fn x_visual_info(
    xcb_connection: &XcbConnection,
    fb_config: *mut glx::__GLXFBConfigRec
) -> *const xlib::XVisualInfo {
    // The GLX framebuffer config holds an XVisualInfo, which we'll need for other X operations.
    
    unsafe { glx::glXGetVisualFromFBConfig(
        xcb_connection.conn.get_raw_dpy(),
        fb_config
    )}
}

pub fn glx_context(
    xcb_connection: &XcbConnection,
    fb_config: *mut glx::__GLXFBConfigRec
) -> *mut glx::__GLXcontextRec {
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
    let glx_create_context_attribs: GlXCreateContextAttribsARBProc =
        unsafe { std::mem::transmute(load_gl_func("glXCreateContextAttribsARB")) };

    // Load all other symbols
    unsafe {
        gl::load_with(|n| load_gl_func(&n));
    }

    // Check GL3 support
    if !gl::GenVertexArrays::is_loaded() {
        panic!("no GL3 support available!");
    }

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

    ctx
}

pub type GlXCreateContextAttribsARBProc = unsafe extern "C" fn(
    dpy: *mut xlib::Display,
    fbc: glx::GLXFBConfig,
    share_context: glx::GLXContext,
    direct: xlib::Bool,
    attribs: *const c_int,
) -> glx::GLXContext;

// Check to make sure this system supports the correct version of GLX (>= 1.3 for now)
// For now it just panics if not, but TODO: do correct error handling
fn check_glx_version(xcb_connection: &XcbConnection) {
    let raw_display = xcb_connection.conn.get_raw_dpy();
    let mut maj: c_int = 0;
    let mut min: c_int = 0;

    unsafe {
        if glx::glXQueryVersion(raw_display, &mut maj as *mut c_int, &mut min as *mut c_int) == 0 {
            panic!("Cannot get GLX version");
        }
        if (maj < 1) || (maj == 1 && min < 3) {
            panic!("GLX version >= 1.3 required! (have {}.{})", maj, min);
        }
    }
}

// Get GLX framebuffer config
// History: https://stackoverflow.com/questions/51558473/whats-the-difference-between-a-glx-visual-and-a-fbconfig
fn get_glxfbconfig(xcb_connection: &XcbConnection, visual_attribs: &[i32]) -> glx::GLXFBConfig {
    let raw_display = xcb_connection.conn.get_raw_dpy();
    let xlib_display = xcb_connection.xlib_display;

    unsafe {
        let mut fbcount: c_int = 0;
        let fbcs = glx::glXChooseFBConfig(
            raw_display,
            xlib_display,
            visual_attribs.as_ptr(),
            &mut fbcount as *mut c_int,
        );

        if fbcount == 0 {
            panic!("Could not find compatible GLX FB config.");
        }

        // If we get more than one, any of the different configs work. Just choose the first one.
        let fbc = *fbcs;
        xlib::XFree(fbcs as *mut c_void);
        fbc
    }
}

pub unsafe fn load_gl_func(name: &str) -> *mut c_void {
    let cname = CString::new(name).unwrap();
    let ptr: *mut c_void = std::mem::transmute(glx::glXGetProcAddress(cname.as_ptr() as *const u8));
    if ptr.is_null() {
        panic!("could not load {}", name);
    }
    ptr
}

pub fn xcb_expose(
    window_id: u32,
    raw_display: *mut xlib::_XDisplay,
    ctx: *mut glx::__GLXcontextRec,
) {
    unsafe {
        glx::glXMakeCurrent(raw_display, window_id as xlib::XID, ctx);
        gl::ClearColor(0.3, 0.8, 0.3, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::Flush();
        glx::glXSwapBuffers(raw_display, window_id as xlib::XID);
        glx::glXMakeCurrent(raw_display, 0, null_mut());
    }
}