use crate::{WindowInfo, Parent, Size, PhySize};

/// The size of the window
#[derive(Debug)]
pub enum WindowSize {
    /// Use logical width and height
    Logical(Size),
    /// Use physical width and height
    Physical(PhySize),
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
        }
    }
}