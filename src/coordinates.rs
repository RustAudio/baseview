use crate::WindowInfo;

/// A point in logical coordinates
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64
}

impl Point {
    /// Create a new point in logical coordinates
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Convert to actual physical coordinates
    #[inline]
    pub fn to_physical(&self, window_info: &WindowInfo) -> PhyPoint {
        PhyPoint {
            x: (self.x * window_info.scale()).round() as i32,
            y: (self.y * window_info.scale()).round() as i32,
        }
    }
}

/// A point in actual physical coordinates
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhyPoint {
    pub x: i32,
    pub y: i32
}

impl PhyPoint {
    /// Create a new point in actual physical coordinates
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Convert to logical coordinates
    #[inline]
    pub fn to_logical(&self, window_info: &WindowInfo) -> Point {
        Point {
            x: f64::from(self.x) * window_info.scale_recip(),
            y: f64::from(self.y) * window_info.scale_recip(),
        }
    }
}

/// A size in logical coordinates
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    /// Create a new size in logical coordinates
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Convert to actual physical size
    #[inline]
    pub fn to_physical(&self, window_info: &WindowInfo) -> PhySize {
        PhySize {
            width: (self.width * window_info.scale()).round() as u32,
            height: (self.height * window_info.scale()).round() as u32,
        }
    }
}

/// An actual size in physical coordinates
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhySize {
    pub width: u32,
    pub height: u32,
}

impl PhySize {
    /// Create a new size in actual physical coordinates
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Convert to logical size
    #[inline]
    pub fn to_logical(&self, window_info: &WindowInfo) -> Size {
        Size {
            width: f64::from(self.width) * window_info.scale_recip(),
            height: f64::from(self.height) * window_info.scale_recip(),
        }
    }
}