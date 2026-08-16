use crate::dpi::Size;
use crate::{AspectRatio, WindowSettings, WindowSize};
use dpi::{LogicalSize, PhysicalSize};

#[derive(Copy, Clone)]
pub(crate) enum SizingStrategy {
    Fixed,
    Resizable { min_size: Option<Size>, max_size: Option<Size>, aspect_ratio: Option<AspectRatio> },
}

impl SizingStrategy {
    pub fn from_settings(settings: &WindowSettings) -> Self {
        if !settings.resizable {
            return Self::Fixed;
        }

        if let (Some(min_size), Some(max_size)) = (settings.min_size, settings.max_size) {
            if min_size == max_size {
                return Self::Fixed;
            }
        }

        let mut min_size = settings.min_size;
        let mut max_size = settings.max_size;

        if let Some(ratio) = settings.aspect_ratio {
            min_size = min_size.map(|s| adjust_min_size(s, ratio));
            max_size = max_size.map(|s| adjust_max_size(s, ratio));
        }

        Self::Resizable { min_size, max_size, aspect_ratio: settings.aspect_ratio }
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        matches!(self, Self::Resizable { .. })
    }

    #[inline]
    pub fn min_size(&self) -> Option<Size> {
        match self {
            Self::Fixed => None,
            Self::Resizable { min_size, .. } => *min_size,
        }
    }

    #[inline]
    pub fn max_size(&self) -> Option<Size> {
        match self {
            Self::Fixed => None,
            Self::Resizable { max_size, .. } => *max_size,
        }
    }

    fn adjust_size_physical(&self, mut size: PhysicalSize<u32>, scale_factor: f64) -> WindowSize {
        if let Some(max_size) = self.max_size() {
            let max_size = max_size.to_physical::<u32>(scale_factor);
            size.width = max_size.width.min(size.width);
            size.height = max_size.height.min(size.height);
        }

        if let Some(min_size) = self.min_size() {
            let min_size = min_size.to_physical::<u32>(scale_factor);
            size.width = min_size.width.max(size.width);
            size.height = min_size.height.max(size.height);
        }

        WindowSize::from_physical(size, scale_factor)
    }

    fn adjust_size_logical(&self, mut size: LogicalSize<f64>, scale_factor: f64) -> WindowSize {
        if let Some(max_size) = self.max_size() {
            let max_size = max_size.to_logical::<f64>(scale_factor);
            size.width = max_size.width.min(size.width);
            size.height = max_size.height.min(size.height);
        }

        if let Some(min_size) = self.min_size() {
            let min_size = min_size.to_logical::<f64>(scale_factor);
            size.width = min_size.width.max(size.width);
            size.height = min_size.height.max(size.height);
        }

        WindowSize::from_logical(size, scale_factor)
    }

    pub fn adjust_size(&self, size: Size, window_size: WindowSize) -> WindowSize {
        if !self.is_resizable() {
            return window_size;
        }

        match size {
            Size::Physical(size) => self.adjust_size_physical(size, window_size.scale_factor),
            Size::Logical(size) => self.adjust_size_logical(size, window_size.scale_factor),
        }
    }

    pub fn aspect_ratio(&self) -> Option<AspectRatio> {
        match self {
            Self::Fixed => None,
            Self::Resizable { aspect_ratio, .. } => *aspect_ratio,
        }
    }
}

impl Default for SizingStrategy {
    fn default() -> Self {
        Self::Resizable { min_size: None, max_size: None, aspect_ratio: None }
    }
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

    match size {
        Size::Physical(PhysicalSize { width, height }) => {
            Size::Physical(PhysicalSize::from(inner(width, height, ratio)))
        }
        Size::Logical(LogicalSize { width, height }) => {
            Size::Logical(LogicalSize::from(inner(width, height, ratio)))
        }
    }
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
