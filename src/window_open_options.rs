#[cfg(feature = "opengl")]
use crate::gl::GlConfig;
use crate::platform;
use dpi::{LogicalSize, Size};
use raw_window_handle::HasWindowHandle;

/// Settings used when creating a new window
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WindowSettings {
    /// The window title
    pub title: String,

    /// The size of the window, either in physical or logical coordinates
    pub size: Size,

    /// If the window is to be embedded in a parent window, the handle to that window.
    ///
    /// If `None`, the window will be standalone.
    pub parent: Option<ParentWindowHandle>,

    /// If provided, then an OpenGL context will be created for this window. You'll be able to
    /// access this context through [crate::WindowContext::gl_context].
    ///
    /// By default, this is set to `None`.
    #[cfg(feature = "opengl")]
    pub gl_config: Option<GlConfig>,
}

impl WindowSettings {
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
    pub fn with_parent<'a, P: HasWindowHandle + 'a>(
        mut self, parent: impl Into<Option<&'a P>>,
    ) -> Self {
        self.parent = parent.into().map(ParentWindowHandle::from_window);
        self
    }

    #[cfg(feature = "opengl")]
    #[inline]
    pub fn with_gl_config(mut self, gl_config: impl Into<Option<GlConfig>>) -> Self {
        self.gl_config = gl_config.into();
        self
    }
}

impl Default for WindowSettings {
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

/// An owned handle to a parent window.
///
/// This type holds just what's needed for baseview to create a child window into this window.
///
/// This can safely be constructed from only a temporary reference to any [`HasWindowHandle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentWindowHandle {
    pub(crate) inner: platform::ParentWindowHandle,
}

impl ParentWindowHandle {
    /// Grabs a handle to the given `parent_window`, to later create a child window in it.
    pub fn from_window(parent_window: &impl HasWindowHandle) -> Self {
        let inner = match platform::ParentWindowHandle::extract(parent_window) {
            Ok(parent) => parent,
            Err(e) => {
                panic!("Invalid parent window handle: {e}")
            }
        };

        Self { inner }
    }
}

impl<W: HasWindowHandle> From<&W> for ParentWindowHandle {
    fn from(window: &W) -> Self {
        Self::from_window(window)
    }
}
