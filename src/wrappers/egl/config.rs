use super::*;
use crate::gl::GlConfig;
use crate::wrappers::egl::display::EglDisplay;
use crate::wrappers::egl::sys::*;
use std::ffi::c_void;
use std::ptr::NonNull;
use x11rb::protocol::xproto::Visualid;

pub struct EglConfig(pub(super) NonNull<c_void>);

impl EglConfig {
    pub(super) fn choose_config(
        gl_config: &GlConfig, display: &EglDisplay,
    ) -> Result<Option<EglConfig>, EglError> {
        let mut config = core::ptr::null_mut();
        let fb_attribs = get_fb_attribs(gl_config);
        let mut num_configs = 0;
        let result = unsafe {
            (display.egl.inner.functions.eglChooseConfig)(
                display.raw.as_ptr(),
                fb_attribs.as_ptr(),
                &mut config,
                1,
                &mut num_configs,
            )
        };

        if result == FALSE {
            return Err(EglError::from_last_error(&display.egl));
        }

        if num_configs == 0 {
            return Ok(None);
        }

        let Some(raw) = NonNull::new(config) else { return Ok(None) };

        Ok(Some(EglConfig(raw)))
    }

    fn get_attrib(&self, display: &EglDisplay, attrib: Int) -> Result<Int, EglError> {
        let mut value = 0;
        let result = unsafe {
            (display.egl.inner.functions.eglGetConfigAttrib)(
                display.raw.as_ptr(),
                self.0.as_ptr(),
                attrib,
                &mut value,
            )
        };

        if result == FALSE {
            return Err(EglError::from_last_error(&display.egl));
        }

        Ok(value)
    }

    pub fn get_visual_id(&self, display: &EglDisplay) -> Result<Visualid, EglError> {
        let value = self.get_attrib(display, EGL_NATIVE_VISUAL_ID)?;
        Ok(value as _) // TODO: cast
    }
}

fn get_fb_attribs(config: &GlConfig) -> [Int; 17] {
    let Some(color_size) = (config.red_bits as i32)
        .checked_add(config.blue_bits as i32)
        .and_then(|c| c.checked_add(config.green_bits as i32))
    else {
        panic!("Overflow when computing color size")
    };

    #[rustfmt::skip]
    let fb_attribs = [
        EGL_BUFFER_SIZE, color_size,
        EGL_RED_SIZE, config.red_bits.into(),
        EGL_GREEN_SIZE, config.green_bits.into(),
        EGL_BLUE_SIZE, config.blue_bits.into(),
        EGL_ALPHA_SIZE, config.alpha_bits.into(),
        EGL_DEPTH_SIZE, config.depth_bits.into(),
        EGL_STENCIL_SIZE, config.stencil_bits.into(),
        EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
        EGL_NONE
    ];

    fb_attribs
}
