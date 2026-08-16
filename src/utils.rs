use crate::{AspectRatio, WindowSettings};
use dpi::Size;

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

        Self::Resizable {
            min_size: settings.min_size,
            max_size: settings.max_size,
            aspect_ratio: settings.aspect_ratio,
        }
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
