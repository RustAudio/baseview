use crate::{Size, Point};

/// The info about the window
#[derive(Debug, Copy, Clone)]
pub struct WindowInfo {
    logical_size: Size,
    physical_size: Size,
    scale: f64,
    scale_recip: f64,
}

impl WindowInfo {
    pub fn from_logical_size(logical_size: Size, scale: f64) -> Self {
        let (scale_recip, physical_size) = if scale == 1.0 {
            (1.0, logical_size)
        } else {
            (
                1.0 / scale,
                Size {
                    width: (logical_size.width as f64 * scale).round() as u32,
                    height: (logical_size.height as f64 * scale).round() as u32,
                }
            )
        };

        Self {
            logical_size,
            physical_size,
            scale,
            scale_recip,
        }
    }

    pub fn from_physical_size(physical_size: Size, scale: f64) -> Self {
        let (scale_recip, logical_size) = if scale == 1.0 {
            (1.0, physical_size)
        } else {
            let scale_recip = 1.0 / scale;
            (
                scale_recip,
                Size {
                    width: (physical_size.width as f64 * scale_recip).round() as u32,
                    height: (physical_size.height as f64 * scale_recip).round() as u32,
                }
            )
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
    pub fn physical_size(&self) -> Size {
        self.physical_size
    }

    /// The scale factor of the window
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Convert physical coordinates to logical coordinates
    pub fn physical_to_logical(&self, physical: Point<f64>) -> Point<f64> {
        Point {
            x: physical.x * self.scale_recip,
            y: physical.y * self.scale_recip
        }
    }

    /// Convert logical coordinates to physical coordinates
    pub fn logical_to_physical(&self, logical: Point<f64>) -> Point<f64> {
        Point {
            x: logical.x * self.scale,
            y: logical.y * self.scale
        }
    }
}