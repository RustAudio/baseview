use crate::{WindowInfo, Parent, Size};

/// The size of the window
#[derive(Debug)]
pub enum WindowSize {
    /// Use logical width and height
    Logical(Size),
    /// Use physical width and height
    Physical(Size),
    /// Use minimum and maximum logical width and height
    MinMaxLogical {
        /// The initial logical width and height
        initial_size: Size,
        /// The minimum logical width and height
        min_size: Size,
        /// The maximum logical width and height
        max_size: Size,
        /// Whether to keep the aspect ratio when resizing (true), or not (false)
        keep_aspect: bool,
    },
    /// Use minimum and maximum physical width and height
    MinMaxPhysical {
        /// The initial physical width and height
        initial_size: Size,
        /// The minimum physical width and height
        min_size: Size,
        /// The maximum physical width and height
        max_size: Size,
        /// Whether to keep the aspect ratio when resizing (true), or not (false)
        keep_aspect: bool,
    },
}

/// The dpi scaling policy of the window
#[derive(Debug)]
pub enum WindowScalePolicy {
    /// Try using the system scale factor
    TrySystemScaleFactor,
    /// Try using the system scale factor in addition to the given scale factor
    TrySystemScaleFactorTimes(f64),
    /// Use the given scale factor
    UseScaleFactor(f64),
    /// No scaling
    NoScaling,
}

/// The options for opening a new window
#[derive(Debug)]
pub struct WindowOpenOptions {
    pub title: String,

    /// The size information about the window
    pub size: WindowSize,

    /// The scaling of the window
    pub scale: WindowScalePolicy,

    pub parent: Parent,
}

impl WindowOpenOptions {
    pub(crate) fn window_info_from_scale(&self, scale: f64) -> WindowInfo {
        match self.size {
            WindowSize::Logical(size) => WindowInfo::from_logical_size(size, scale),
            WindowSize::Physical(size) => WindowInfo::from_physical_size(size, scale),
            WindowSize::MinMaxLogical { initial_size, .. } => {
                WindowInfo::from_logical_size(initial_size, scale)
            },
            WindowSize::MinMaxPhysical { initial_size, .. } => {
                WindowInfo::from_logical_size(initial_size, scale)
            }
        }
    }
}