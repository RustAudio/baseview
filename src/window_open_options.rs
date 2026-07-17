#[cfg(feature = "opengl")]
use crate::gl::GlConfig;
use crate::platform::ParentWindowHandle;
use dpi::{LogicalSize, Size};
use raw_window_handle::HasWindowHandle;

/// The options for opening a new window
#[derive(Debug, Clone, PartialEq)]
pub struct WindowOpenOptions {
    pub title: String,

    /// The size of the window, either in physical or logical coordinates
    pub size: Size,

    pub(crate) parent: Option<ParentWindowHandle>,

    /// If provided, then an OpenGL context will be created for this window. You'll be able to
    /// access this context through [crate::WindowContext::gl_context].
    ///
    /// By default, this is set to `None`.
    #[cfg(feature = "opengl")]
    pub gl_config: Option<GlConfig>,
}

impl WindowOpenOptions {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    #[inline]
    pub fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }

    #[inline]
    pub fn with_parent(mut self, parent: &impl HasWindowHandle) -> Self {
        let parent = match ParentWindowHandle::extract(parent) {
            Ok(parent) => parent,
            Err(e) => {
                panic!("Invalid parent window handle: {e}")
            }
        };

        self.parent = Some(parent);
        self
    }

    #[cfg(feature = "opengl")]
    #[inline]
    pub fn with_gl_config(mut self, gl_config: impl Into<Option<GlConfig>>) -> Self {
        self.gl_config = gl_config.into();
        self
    }
}

impl Default for WindowOpenOptions {
    fn default() -> Self {
        Self {
            title: String::from("baseview window"),
            size: LogicalSize { width: 500.0, height: 400.0 }.into(),
            parent: None,
            #[cfg(feature = "opengl")]
            gl_config: None,
        }
    }
}
