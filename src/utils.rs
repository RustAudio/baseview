use crate::dpi::Size;
use crate::{WindowSettings, WindowSize};
use dpi::{LogicalSize, PhysicalSize};

#[derive(Copy, Clone)]
pub(crate) enum SizingStrategy {
    Fixed,
    Resizable { min_size: Option<Size>, max_size: Option<Size> },
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

        Self::Resizable { min_size: settings.min_size, max_size: settings.max_size }
    }

    pub fn is_resizable(&self) -> bool {
        matches!(self, Self::Resizable { .. })
    }

    pub fn min_size(&self) -> Option<Size> {
        match self {
            Self::Fixed => None,
            Self::Resizable { min_size, .. } => *min_size,
        }
    }

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
}

impl Default for SizingStrategy {
    fn default() -> Self {
        Self::Resizable { min_size: None, max_size: None }
    }
}
