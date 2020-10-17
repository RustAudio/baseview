use crate::{Size, PhySize};

/// The info about the window
#[derive(Debug, Copy, Clone)]
pub struct WindowInfo {
    logical_size: Size,
    physical_size: PhySize,
    scale: f64,
    scale_recip: f64,
}

impl WindowInfo {
    pub fn from_logical_size(logical_size: Size, scale: f64) -> Self {
        let scale_recip = if scale == 1.0 { 1.0 } else { 1.0 / scale };

        let physical_size = PhySize {
            width: (logical_size.width * scale).round() as u32,
            height: (logical_size.height * scale).round() as u32,
        };

        Self {
            logical_size,
            physical_size,
            scale,
            scale_recip,
        }
    }

    pub fn from_physical_size(physical_size: PhySize, scale: f64) -> Self {
        let scale_recip = if scale == 1.0 { 1.0 } else { 1.0 / scale };

        let logical_size = Size {
            width: f64::from(physical_size.width) * scale_recip,
            height: f64::from(physical_size.height) * scale_recip,
        };

        Self {
            logical_size,
            physical_size,
            scale,
            scale_recip,
        }
    }

    /// The logical size of the window
    pub fn logical_size(&self) -> Size {
        self.logical_size
    }

    /// The physical size of the window
    pub fn physical_size(&self) -> PhySize {
        self.physical_size
    }

    /// The scale factor of the window
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The reciprocal of the scale factor of the window
    pub fn scale_recip(&self) -> f64 {
        self.scale_recip
    }
}