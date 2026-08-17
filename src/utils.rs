use crate::{AspectRatio, WindowSettings};
use dpi::{LogicalSize, PhysicalSize, Pixel, Size};

#[derive(Copy, Clone)]
pub(crate) enum SizingStrategy {
    Fixed { size: Size },
    Resizable { min_size: Option<Size>, max_size: Option<Size>, aspect_ratio: Option<AspectRatio> },
}

impl SizingStrategy {
    pub fn from_settings(settings: &WindowSettings) -> Self {
        if !settings.resizable {
            return Self::Fixed { size: settings.size };
        }

        let mut min_size = settings.min_size;
        let mut max_size = settings.max_size;

        if let (Some(min_size), Some(max_size)) = (min_size, max_size) {
            if min_size == max_size {
                return Self::Fixed { size: min_size };
            }
        }

        if let Some(ratio) = settings.aspect_ratio {
            min_size = min_size.map(|s| adjust_min_size(s, ratio));
            max_size = max_size.map(|s| adjust_max_size(s, ratio));
        }

        Self::Resizable { min_size, max_size, aspect_ratio: settings.aspect_ratio }
    }

    pub fn is_resizable(&self) -> bool {
        matches!(self, Self::Resizable { .. })
    }

    pub fn min_size(&self) -> Option<Size> {
        match self {
            Self::Fixed { .. } => None,
            Self::Resizable { min_size, .. } => *min_size,
        }
    }

    pub fn max_size(&self) -> Option<Size> {
        match self {
            Self::Fixed { .. } => None,
            Self::Resizable { max_size, .. } => *max_size,
        }
    }

    pub fn aspect_ratio(&self) -> Option<AspectRatio> {
        match self {
            Self::Fixed { .. } => None,
            Self::Resizable { aspect_ratio, .. } => *aspect_ratio,
        }
    }

    pub fn adjust_size(&self, size: Size, scale_factor: f64) -> Size {
        let (min, max, ratio) = match *self {
            SizingStrategy::Resizable { min_size, max_size, aspect_ratio } => {
                (min_size, max_size, aspect_ratio)
            }
            SizingStrategy::Fixed { size: fixed_size } => {
                return match size {
                    Size::Physical(_) => Size::Physical(fixed_size.to_physical(scale_factor)),
                    Size::Logical(_) => Size::Logical(fixed_size.to_logical(scale_factor)),
                }
            }
        };

        match size {
            Size::Physical(mut size) => {
                if let Some(max_size) = max {
                    let max_size = max_size.to_physical(scale_factor);
                    size.width = size.width.min(max_size.width);
                    size.height = size.height.min(max_size.height);
                }

                if let Some(min_size) = min {
                    let min_size = min_size.to_physical(scale_factor);
                    size.width = size.width.max(min_size.width);
                    size.height = size.height.max(min_size.height);
                }

                if let Some(ratio) = ratio {
                    todo!()
                }

                size.into()
            }
            Size::Logical(size) => {
                todo!()
            }
        }
    }
}

impl Default for SizingStrategy {
    fn default() -> Self {
        Self::Resizable { min_size: None, max_size: None, aspect_ratio: None }
    }
}

macro_rules! map_size {
    ($size:expr, $ratio:expr, $inner:expr) => {
        match $size {
            Size::Physical(PhysicalSize { width, height }) => {
                Size::Physical(PhysicalSize::from($inner(width, height, $ratio)))
            }
            Size::Logical(LogicalSize { width, height }) => {
                Size::Logical(LogicalSize::from($inner(width, height, $ratio)))
            }
        }
    };
}

fn adjust_min_size(size: Size, ratio: AspectRatio) -> Size {
    fn inner<P: Pixel>(x: P, y: P, ratio: AspectRatio) -> (P, P) {
        if ratio.numerator < ratio.denominator {
            let raw_ratio = ratio.ratio();
            let y = y.into();
            let y = y.max(x.into() / raw_ratio);
            (x, y.cast())
        } else if ratio.numerator == ratio.denominator {
            if x.into() > y.into() {
                (x, x)
            } else {
                (y, y)
            }
        } else {
            let raw_ratio = ratio.ratio();
            let x = x.into();
            let x = x.max(y.into() * raw_ratio);
            (x.cast(), y)
        }
    }

    match size {
        Size::Physical(PhysicalSize { width, height }) => {
            Size::Physical(PhysicalSize::from(inner(width, height, ratio)))
        }
        Size::Logical(LogicalSize { width, height }) => {
            Size::Logical(LogicalSize::from(inner(width, height, ratio)))
        }
    }
}

fn adjust_max_size(size: Size, ratio: AspectRatio) -> Size {
    fn inner<P: Pixel>(x: P, y: P, ratio: AspectRatio) -> (P, P) {
        if ratio.numerator < ratio.denominator {
            let raw_ratio = ratio.ratio();
            let x = x.into();
            let x = x.min(y.into() * raw_ratio);
            (x.cast(), y)
        } else if ratio.numerator == ratio.denominator {
            if x.into() > y.into() {
                (x, x)
            } else {
                (y, y)
            }
        } else {
            let raw_ratio = ratio.ratio();
            let y = y.into();
            let y = y.min(x.into() / raw_ratio);
            (x, y.cast())
        }
    }

    map_size!(size, ratio, inner)
}

#[cfg(test)]
mod tests {
    use crate::utils::SizingStrategy;
    use crate::{AspectRatio, WindowSettings};
    use dpi::LogicalSize;

    #[test]
    fn aspect_ratio_set_to_min_size() {
        let mut settings = WindowSettings::new()
            .with_min_size(LogicalSize::new(200.0, 300.0))
            .with_aspect_ratio(Some((1, 2)));

        assert_eq!(
            SizingStrategy::from_settings(&settings).min_size(),
            Some(LogicalSize::new(200.0, 400.0).into())
        );

        settings.aspect_ratio = Some(AspectRatio::from((2, 1)));

        assert_eq!(
            SizingStrategy::from_settings(&settings).min_size(),
            Some(LogicalSize::new(600.0, 300.0).into())
        );
    }

    #[test]
    fn aspect_ratio_set_to_max_size() {
        let mut settings = WindowSettings::new()
            .with_max_size(LogicalSize::new(200.0, 300.0))
            .with_aspect_ratio(Some((1, 2)));

        assert_eq!(
            SizingStrategy::from_settings(&settings).max_size(),
            Some(LogicalSize::new(150.0, 300.0).into())
        );

        settings.aspect_ratio = Some(AspectRatio::from((2, 1)));

        assert_eq!(
            SizingStrategy::from_settings(&settings).max_size(),
            Some(LogicalSize::new(200.0, 100.0).into())
        );
    }
}
