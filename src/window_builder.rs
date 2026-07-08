use super::*;
use crate::platform::ParentWindowHandle;
use dpi::{LogicalSize, Size};
use raw_window_handle::HasWindowHandle;

#[non_exhaustive]
pub struct WindowBuilder {
    pub title: Option<String>,
    pub size: Size,
    parented: bool,
    parent: Option<ParentWindowHandle>,
    #[cfg(feature = "opengl")]
    gl_config: Option<gl::GlConfig>,
}

impl WindowBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[cfg(feature = "opengl")]
    pub fn with_gl(mut self) -> Self {
        self.gl_config = Some(crate::gl::GlConfig::default());
        self
    }

    #[cfg(feature = "opengl")]
    pub fn with_gl_config(mut self, config: crate::gl::GlConfig) -> Self {
        self.gl_config = Some(config);
        self
    }

    pub fn parented(mut self) -> Self {
        self.parented = true;
        self
    }

    pub fn with_parent(mut self, parent: &impl HasWindowHandle) -> Self {
        self.parent = Some(ParentWindowHandle::extract(parent));
        self.parented = true;
        self
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self {
            title: None,
            size: LogicalSize::new(420.0, 240.0).into(),
            parent: None,
            parented: false,
            #[cfg(feature = "opengl")]
            gl_config: None,
        }
    }
}
