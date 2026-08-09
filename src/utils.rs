use crate::WindowSettings;
use dpi::Size;

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
}

impl Default for SizingStrategy {
    fn default() -> Self {
        Self::Resizable { min_size: None, max_size: None }
    }
}
