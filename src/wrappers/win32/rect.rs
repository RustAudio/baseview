use crate::dpi::PhysicalSize;
use std::fmt::Debug;
use windows_sys::Win32::Foundation::RECT;

#[derive(Copy, Clone)]
pub struct Rect(pub RECT);

impl Rect {
    pub const EMPTY: Self = Self(RECT { left: 0, top: 0, right: 0, bottom: 0 });

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.0.right.abs_diff(self.0.left),
            height: self.0.top.abs_diff(self.0.bottom),
        }
    }
}

impl From<PhysicalSize<u32>> for Rect {
    fn from(size: PhysicalSize<u32>) -> Self {
        Self(RECT {
            left: 0,
            top: 0,
            right: size.width.try_into().unwrap_or(i32::MAX),
            bottom: size.height.try_into().unwrap_or(i32::MAX),
        })
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
