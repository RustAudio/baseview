use crate::dpi::PhysicalSize;
use crate::HandlerError;
use std::fmt::Display;

pub type Result<T> = std::result::Result<T, PlatformError>;

#[derive(Debug)]
pub enum PlatformError {
    Win32(windows_core::Error),
    UnexpectedResizeResult { new_actual_size: PhysicalSize<u32> },
    InvalidScaleFactor,
    Handler(HandlerError),
}

impl From<windows_core::Error> for PlatformError {
    fn from(value: windows_core::Error) -> Self {
        PlatformError::Win32(value)
    }
}

impl From<HandlerError> for PlatformError {
    fn from(value: HandlerError) -> Self {
        PlatformError::Handler(value)
    }
}

impl Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::Win32(e) => Display::fmt(e, f),
            PlatformError::Handler(e) => Display::fmt(e, f),
            PlatformError::UnexpectedResizeResult { new_actual_size } => {
                write!(
                    f,
                    "Resize operation did not result in expected size, got {:?} instead.",
                    new_actual_size
                )
            }
            PlatformError::InvalidScaleFactor => f.write_str("Invalid window scale factor."),
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformError::Win32(e) => Some(e),
            PlatformError::Handler(e) => Some(e.source()),
            _ => None,
        }
    }
}
