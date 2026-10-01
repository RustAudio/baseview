use super::*;
use crate::dpi::{PhysicalPosition, PhysicalSize};
use std::fmt::{Debug, Formatter};

/// Describes the area(s) of a window that have been damaged and need to be redrawn.
#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum DamageArea<'a> {
    /// The full window needs to be redrawn.
    FullWindow,
    /// Damages a single, specific [rectangle](DamageRect).
    Rect(DamageRect),
    /// Damages multiple, potentially overlapping [rectangles](DamageRect).
    Rects(&'a [DamageRect]),
}

/// A damaged rectangle.
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct DamageRect {
    inner: platform::DamageRect,
}

impl DamageRect {
    /// Returns the position of the top-left corner of the damaged rectangle, in physical pixels.
    #[inline]
    pub fn position(&self) -> PhysicalPosition<u32> {
        self.inner.position()
    }

    /// Returns the size of the damaged rectangle, in physical pixels.
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.inner.size()
    }
}

impl From<platform::DamageRect> for DamageRect {
    #[inline]
    fn from(value: platform::DamageRect) -> Self {
        DamageRect { inner: value }
    }
}

impl Debug for DamageRect {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
