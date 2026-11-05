use crate::dpi::Size;
use crate::{AspectRatio, WindowSettings, WindowSize};
use dpi::{LogicalSize, PhysicalSize, Pixel};

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

    #[inline]
    pub fn is_resizable(&self) -> bool {
        matches!(self, Self::Resizable { .. })
    }

    #[inline]
    pub fn min_size(&self) -> Option<Size> {
        match self {
            Self::Fixed { .. } => None,
            Self::Resizable { min_size, .. } => *min_size,
        }
    }

    #[inline]
    pub fn max_size(&self) -> Option<Size> {
        match self {
            Self::Fixed { .. } => None,
            Self::Resizable { max_size, .. } => *max_size,
        }
    }

    pub(crate) fn adjust_size_physical(
        &self, mut size: PhysicalSize<u32>, scale_factor: f64,
    ) -> WindowSize {
        if let Some(max_size) = self.max_size() {
            let max_size = max_size.to_physical::<u32>(scale_factor);
            size.width = max_size.width.min(size.width);
            size.height = max_size.height.min(size.height);
        }

        if let Some(aspect_ratio) = self.aspect_ratio() {
            let given_ratio = size.width as f64 / size.height as f64;
            let required_ratio = aspect_ratio.ratio();

            let biggest_fitting_size = if given_ratio > required_ratio {
                PhysicalSize {
                    width: (size.height as f64 * required_ratio).cast(),
                    height: size.height,
                }
            } else {
                PhysicalSize {
                    width: size.width,
                    height: (size.width as f64 / required_ratio).cast(),
                }
            };

            size = biggest_fitting_size;
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

        if let Some(aspect_ratio) = self.aspect_ratio() {
            let given_ratio = size.width / size.height;
            let required_ratio = aspect_ratio.ratio();

            let biggest_fitting_size = if given_ratio > required_ratio {
                LogicalSize { width: (size.height * required_ratio).cast(), height: size.height }
            } else {
                LogicalSize { width: size.width, height: (size.width / required_ratio).cast() }
            };

            size = biggest_fitting_size;
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
            Self::Fixed { .. } => None,
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
    use crate::dpi::*;
    use crate::utils::SizingStrategy;
    use crate::{AspectRatio, WindowSettings, WindowSize};
    use std::cell::Cell;

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

    #[test]
    fn adjust_size() {
        let settings = WindowSettings::new()
            .with_min_size(LogicalSize::new(200.0, 300.0))
            .with_max_size(LogicalSize::new(400.0, 800.0));

        let current_window_size = WindowSize::from_logical(LogicalSize::new(200.0, 400.0), 1.0);

        let strategy = Cell::new(SizingStrategy::from_settings(&settings));

        let assert_size = |input: (u32, u32), expected: (u32, u32)| {
            let input = NativeSize::new(input.0, input.1);
            let expected = NativeSize::new(expected.0, expected.1);

            assert_eq!(
                strategy.get().adjust_size(input.into(), current_window_size).to_native_size(),
                expected
            );
        };

        // Check min
        assert_size((250, 450), (250, 450));
        assert_size((50, 450), (200, 450));
        assert_size((50, 50), (200, 300));

        // Check max
        assert_size((5000, 450), (400, 450));
        assert_size((250, 5000), (250, 800));
        assert_size((5000, 5000), (400, 800));

        // Check conflicting min&max
        let settings = settings.with_max_size(LogicalSize::new(150.0, 200.0));
        strategy.set(SizingStrategy::from_settings(&settings));

        assert_size((5000, 5000), (200, 300));
    }

    #[test]
    fn adjust_size_with_aspect_ratio() {
        let current_window_size = WindowSize::from_logical(LogicalSize::new(200.0, 400.0), 1.0);

        let settings = WindowSettings::new().with_aspect_ratio(Some((1, 1)));
        let strategy = Cell::new(SizingStrategy::from_settings(&settings));

        let assert_size = |input: (u32, u32), expected: (u32, u32)| {
            let input = NativeSize::new(input.0, input.1);
            let expected = NativeSize::new(expected.0, expected.1);

            assert_eq!(
                strategy.get().adjust_size(input.into(), current_window_size).to_native_size(),
                expected
            );
        };

        assert_size((200, 200), (200, 200));

        // Should always shrink, never grow
        assert_size((500, 300), (300, 300));
        assert_size((300, 500), (300, 300));
    }
}
