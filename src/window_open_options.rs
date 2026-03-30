use crate::Size;

#[cfg(feature = "opengl")]
use crate::gl::GlConfig;

/// The dpi scaling policy of the window
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum WindowScalePolicy {
    /// Use the system's dpi scale factor
    #[default]
    SystemScaleFactor,
    /// Use the given dpi scale factor (e.g. `1.0` = 96 dpi)
    ScaleFactor(f64),
}

/// The options for opening a new window
#[derive(Debug, Clone, PartialEq)]
pub struct WindowOpenOptions {
    pub title: String,

    /// The logical size of the window.
    ///
    /// These dimensions will be scaled by the scaling policy specified in `scale`. Mouse
    /// position will be passed back as logical coordinates.
    pub size: Size,

    /// The dpi scaling policy
    pub scale: WindowScalePolicy,

    /// If provided, then an OpenGL context will be created for this window. You'll be able to
    /// access this context through [crate::Window::gl_context].
    ///
    /// By default this is set to `Some(GlConfig::default())`.
    #[cfg(feature = "opengl")]
    pub gl_config: Option<GlConfig>,
}

impl WindowOpenOptions {
    pub fn default_no_opengl() -> Self {
        Self {
            title: String::from("baseview window"),
            size: Size { width: 500.0, height: 400.0 },
            scale: WindowScalePolicy::default(),
            #[cfg(feature = "opengl")]
            gl_config: None,
        }
    }
}

impl Default for WindowOpenOptions {
    fn default() -> Self {
        Self {
            title: String::from("baseview window"),
            size: Size { width: 500.0, height: 400.0 },
            scale: WindowScalePolicy::default(),
            #[cfg(feature = "opengl")]
            gl_config: Some(GlConfig::default()),
        }
    }
}
