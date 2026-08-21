#[cfg(feature = "opengl")]
use crate::gl::GlConfig;
use crate::platform;
use dpi::{LogicalSize, Size};
use raw_window_handle::HasWindowHandle;

/// Settings used when creating a new window.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WindowSettings {
    /// The window title.
    pub title: String,

    /// The size of the window, either in physical or logical coordinates.
    pub size: Size,

    /// If the window is to be embedded in a parent window, the handle to that window.
    ///
    /// If `None`, the window will be standalone.
    pub parent: Option<ParentWindowHandle>,

    /// If the window expects to have a parent when first displayed.
    ///
    /// Setting this will delay the actual creation of the window until the parent is set (unless
    /// the window is shown first).
    ///
    /// If the `parent` field is already set, this does nothing and is ignored.
    pub wait_for_parent: bool,

    /// Whether the window can be resized.
    pub resizable: bool,

    pub min_size: Option<Size>,
    pub max_size: Option<Size>,

    /// A fallback scale factor, if Baseview couldn't get one from the platform.
    ///
    /// If the platform does already provide an accurate scaling factor, this doesn't do anything.
    ///
    /// If the given fallback scale factor is actually useful and different from the current one
    /// (1.0 by default), this will resize and redraw the window accordingly.
    ///
    /// # Platform compatibility notes.
    ///
    /// On Win32, this value is used if running on early versions of Windows 10 (or earlier).
    ///
    /// On X11, this value is used if no `Xft.dpi`setting is set.
    ///
    /// On macOS, this function is always a no-op.
    pub fallback_scale_factor: Option<f64>,

    pub redraw_strategy: RedrawStrategy,

    /// If provided, then an OpenGL context will be created for this window. You'll be able to
    /// access this context through [crate::WindowContext::gl_context].
    ///
    /// By default, this is set to `None`.
    #[cfg(feature = "opengl")]
    pub gl_config: Option<GlConfig>,
}

impl WindowSettings {
    /// Creates a new [`WindowSettings`] with all default values.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [`title`](Self::title) to the given value.
    #[inline]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets [`size`](Self::size) to the given value.
    #[inline]
    pub fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }

    /// Sets [`size`](Self::size) to the given value.
    #[inline]
    pub fn with_parent<'a, P: HasWindowHandle + 'a>(
        mut self, parent: impl Into<Option<&'a P>>,
    ) -> Self {
        self.parent = parent.into().map(ParentWindowHandle::from_window);
        self
    }

    /// Sets [`wait_for_parent`](Self::wait_for_parent) to `true`.
    #[inline]
    pub fn wait_for_parent(mut self) -> Self {
        self.wait_for_parent = true;
        self
    }

    /// Sets [`wait_for_parent`](Self::wait_for_parent) to the given value.
    pub fn with_wait_for_parent(mut self, wait_for_parent: bool) -> Self {
        self.wait_for_parent = wait_for_parent;
        self
    }

    /// Sets [`fallback_scale_factor`](Self::fallback_scale_factor) to the given value.
    #[inline]
    pub fn with_fallback_scale_factor(mut self, scale_factor: impl Into<Option<f64>>) -> Self {
        self.fallback_scale_factor = scale_factor.into();
        self
    }

    /// Sets [`resizable`](Self::resizable) to the given value.
    #[inline]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    #[inline]
    pub fn with_min_size<S: Into<Size>>(mut self, min_size: impl Into<Option<S>>) -> Self {
        self.min_size = min_size.into().map(S::into);
        self
    }

    #[inline]
    pub fn with_max_size<S: Into<Size>>(mut self, max_size: impl Into<Option<S>>) -> Self {
        self.max_size = max_size.into().map(S::into);
        self
    }

    #[inline]
    pub fn with_redraw_strategy(mut self, redraw_strategy: RedrawStrategy) -> Self {
        self.redraw_strategy = redraw_strategy;
        self
    }

    /// Sets [`gl_config`](Self::gl_config) to the given value.
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
            wait_for_parent: false,
            fallback_scale_factor: None,
            resizable: true,
            min_size: None,
            max_size: None,
            redraw_strategy: RedrawStrategy::Continuous,
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

// Assert this is Send+Sync
const _: () = {
    const fn foo<T: Send + Sync>() {}
    foo::<ParentWindowHandle>();
};

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

impl From<platform::ParentWindowHandle> for ParentWindowHandle {
    fn from(inner: platform::ParentWindowHandle) -> Self {
        Self { inner }
    }
}

#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RedrawStrategy {
    Continuous,
    OnDemand,
}
