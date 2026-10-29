use crate::dpi::PhysicalSize;
use dpi::PhysicalPosition;
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
    pub fn with_size_anchor_top_left(mut self, size: PhysicalSize<u32>) -> Self {
        self.0.right = self.0.left.saturating_add(size.width.try_into().unwrap_or(i32::MAX));
        self.0.bottom = self.0.top.saturating_add(size.height.try_into().unwrap_or(i32::MAX));
        self
    }

    #[inline]
    pub fn with_size_anchor_top_right(mut self, size: PhysicalSize<u32>) -> Self {
        let current_width = self.size().width.try_into().unwrap_or(i32::MAX);

        let size = PhysicalSize {
            width: size.width.try_into().unwrap_or(i32::MAX),
            height: size.height.try_into().unwrap_or(i32::MAX),
        };

        self.0.left = self.0.left.saturating_add(current_width.saturating_sub(size.width));
        self.0.bottom = self.0.top.saturating_add(size.height);

        self
    }

    #[inline]
    pub fn with_size_anchor_bottom_left(mut self, size: PhysicalSize<u32>) -> Self {
        let current_height = self.size().height.try_into().unwrap_or(i32::MAX);

        let size = PhysicalSize {
            width: size.width.try_into().unwrap_or(i32::MAX),
            height: size.height.try_into().unwrap_or(i32::MAX),
        };

        self.0.right = self.0.left.saturating_add(size.width);
        self.0.top = self.0.top.saturating_add(current_height.saturating_sub(size.height));

        self
    }

    #[inline]
    pub fn with_size_anchor_bottom_right(mut self, size: PhysicalSize<u32>) -> Self {
        let current_size = self.size();
        let current_height = current_size.height.try_into().unwrap_or(i32::MAX);
        let current_width = current_size.width.try_into().unwrap_or(i32::MAX);

        let size = PhysicalSize {
            width: size.width.try_into().unwrap_or(i32::MAX),
            height: size.height.try_into().unwrap_or(i32::MAX),
        };

        self.0.left = self.0.left.saturating_add(current_width.saturating_sub(size.width));
        self.0.top = self.0.top.saturating_add(current_height.saturating_sub(size.height));

        self
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

// Replace this with u32::checked_signed_diff from stdlib when MSRV >= 1.91
fn checked_signed_diff(lhs: u32, rhs: u32) -> Option<i32> {
    let res = lhs.wrapping_sub(rhs) as i32;
    let overflow = (lhs >= rhs) == (res < 0);

    if !overflow {
        Some(res)
    } else {
        None
    }
}
