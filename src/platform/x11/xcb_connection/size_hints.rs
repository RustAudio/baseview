use crate::WindowSettings;
use dpi::PhysicalSize;
use x11rb::properties::WmSizeHints;

pub fn get_size_hints(settings: &WindowSettings, scale_factor: f64) -> WmSizeHints {
    let mut size_hints = WmSizeHints::default();

    if !settings.resizable {
        size_hints = size_hints.with_fixed_size(settings.size.to_physical(scale_factor));
    }

    size_hints
}

pub trait WmSizeHintsExt: Sized {
    fn with_fixed_size(self, size: PhysicalSize<i32>) -> Self;
}

impl WmSizeHintsExt for WmSizeHints {
    fn with_fixed_size(mut self, size: PhysicalSize<i32>) -> Self {
        self.max_size = Some((size.width, size.height));
        self
    }
}
