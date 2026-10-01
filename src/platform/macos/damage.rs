use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use objc2_foundation::NSRect;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DamageRect {
    pos: PhysicalPosition<u32>,
    size: PhysicalSize<u32>,
}

impl DamageRect {
    pub(crate) fn from_rect(rect: NSRect, scale_factor: f64) -> Self {
        let pos = LogicalPosition { x: rect.origin.x, y: rect.origin.y };
        let size = LogicalSize { width: rect.size.width, height: rect.size.height };

        Self { pos: pos.to_physical(scale_factor), size: size.to_physical(scale_factor) }
    }
    #[inline]
    pub fn position(&self) -> PhysicalPosition<u32> {
        self.pos
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }
}
