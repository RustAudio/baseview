/// The info about the window
#[derive(Debug, Copy, Clone)]
pub struct WindowInfo {
    /// The physical the width of the window
    physical_width: u32,
    /// The physical height of the window
    physical_height: u32,
    /// The logical width of the window
    logical_width: u32,
    /// The logical height of the window
    logical_height: u32,
    /// The scale factor
    scale: f64,
    scale_recip: f64,
}

impl WindowInfo {
    pub fn from_logical_size(logical_width: u32, logical_height: u32, scale: f64) -> Self {
        let scale_recip = 1.0 / scale;

        Self {
            physical_width: (logical_width as f64 * scale).round() as u32,
            physical_height: (logical_height as f64 * scale).round() as u32,
            logical_width,
            logical_height,
            scale,
            scale_recip,
        }
    }

    pub fn from_physical_size(physical_width: u32, physical_height: u32, scale: f64) -> Self {
        let scale_recip = 1.0 / scale;

        Self {
            physical_width,
            physical_height,
            logical_width: (physical_width as f64 * scale_recip).round() as u32,
            logical_height: (physical_height as f64 * scale_recip).round() as u32,
            scale,
            scale_recip,
        }
    }

    /// The physical width of the window
    pub fn physical_width(&self) -> u32 {
        self.physical_width
    }

    /// The physical height of the window
    pub fn physical_height(&self) -> u32 {
        self.physical_height
    }

    /// The logical width of the window
    pub fn logical_width(&self) -> u32 {
        self.logical_width
    }

    /// The logical height of the window
    pub fn logical_height(&self) -> u32 {
        self.logical_height
    }

    /// The scale factor of the window
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Convert physical coordinates to logical coordinates
    pub fn physical_to_logical(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.scale_recip,
            y * self.scale_recip
        )
    }

    /// Convert logicalcoordinates to physical coordinates
    pub fn logical_to_physical(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.scale,
            y * self.scale
        )
    }
}