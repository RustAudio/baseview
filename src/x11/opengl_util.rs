use std::ffi::CString;
use std::os::raw::{c_int, c_void};

use ::x11::{glx, xlib};

use super::XcbConnection;

pub type GlXCreateContextAttribsARBProc = unsafe extern "C" fn(
    dpy: *mut xlib::Display,
    fbc: glx::GLXFBConfig,
    share_context: glx::GLXContext,
    direct: xlib::Bool,
    attribs: *const c_int,
) -> glx::GLXContext;

// Check to make sure this system supports the correct version of GLX (>= 1.3 for now)
// For now it just panics if not, but TODO: do correct error handling
pub fn check_glx_version(xcb_connection: &XcbConnection) {
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
pub fn get_glxfbconfig(xcb_connection: &XcbConnection, visual_attribs: &[i32]) -> glx::GLXFBConfig {
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
