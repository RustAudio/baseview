use crate::utils::SizingStrategy;
use dpi::{PhysicalSize, Pixel};
use x11rb::properties::WmSizeHints;

pub fn get_size_hints(
    strategy: &SizingStrategy, current_size: PhysicalSize<impl Pixel>, scale_factor: f64,
) -> WmSizeHints {
    let mut size_hints = WmSizeHints::default();

    match strategy {
        SizingStrategy::Fixed => {
            size_hints.min_size = Some(to_size_hint(current_size));
            size_hints.max_size = size_hints.min_size;
        }
        SizingStrategy::Resizable { min_size, max_size } => {
            size_hints.min_size =
                min_size.map(|s| to_size_hint(s.to_physical::<i32>(scale_factor)));
            size_hints.max_size =
                max_size.map(|s| to_size_hint(s.to_physical::<i32>(scale_factor)));
        }
    }

    size_hints
}

fn to_size_hint(size: PhysicalSize<impl Pixel>) -> (i32, i32) {
    let size = size.cast();
    (size.width, size.height)
}
