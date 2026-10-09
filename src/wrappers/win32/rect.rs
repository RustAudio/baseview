use crate::dpi::PhysicalPosition;
use crate::dpi::PhysicalSize;
use std::fmt::Debug;
use windows_sys::Win32::Foundation::RECT;

#[derive(Copy, Clone)]
pub struct Rect(pub RECT);

impl Rect {
    pub const EMPTY: Self = Self(RECT { left: 0, top: 0, right: 0, bottom: 0 });

    #[inline]
    pub fn position(&self) -> PhysicalPosition<u32> {
        PhysicalPosition {
            x: self.0.left.min(self.0.right).try_into().unwrap_or(0),
            y: self.0.top.min(self.0.bottom).try_into().unwrap_or(0),
        }
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.0.right.abs_diff(self.0.left),
            height: self.0.top.abs_diff(self.0.bottom),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        let size = self.size();
        size.width == 0 && size.height == 0
    }
}

impl Debug for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rect")
            .field("left", &self.0.left)
            .field("top", &self.0.top)
            .field("right", &self.0.right)
            .field("bottom", &self.0.bottom)
            .finish()
    }
}

impl PartialEq for Rect {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.left == other.0.left
            && self.0.bottom == other.0.bottom
            && self.0.right == other.0.right
            && self.0.top == other.0.top
    }
}

impl Eq for Rect {}

impl From<PhysicalSize<u32>> for Rect {
    #[inline]
    fn from(size: PhysicalSize<u32>) -> Self {
        Self(RECT {
            left: 0,
            top: 0,
            right: size.width.try_into().unwrap_or(i32::MAX),
            bottom: size.height.try_into().unwrap_or(i32::MAX),
        })
    }
}
