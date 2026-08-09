use crate::WindowSettings;
use dpi::PhysicalSize;

#[cfg(target_os = "linux")]
type NativeSize = PhysicalSize<u16>;

#[derive(Copy, Clone)]
pub(crate) enum SizingStrategy {
    Fixed,
    Resizable { min_size: Option<NativeSize>, max_size: Option<NativeSize> },
}

impl SizingStrategy {
    pub fn from_settings(settings: &WindowSettings, scale_factor: f64) -> Self {
        if !settings.resizable {
            return Self::Fixed;
        }

        Self::Resizable {
            min_size: settings.min_size.map(|s| s.to_physical(scale_factor)),
            max_size: settings.max_size.map(|s| s.to_physical(scale_factor)),
        }
    }

    pub fn is_resizable(&self) -> bool {
        matches!(self, Self::Resizable { .. })
    }

    pub fn min_size(&self) -> Option<NativeSize> {
        match self {
            Self::Fixed => None,
            Self::Resizable { min_size, .. } => *min_size,
        }
    }

    pub fn max_size(&self) -> Option<NativeSize> {
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
