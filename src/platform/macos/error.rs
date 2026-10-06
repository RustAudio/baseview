use crate::HandlerError;
use std::fmt::Display;

#[derive(Debug)]
pub enum PlatformError {
    Handler(HandlerError),
    InvalidWindowContext,
    #[cfg(feature = "opengl")]
    GlError(super::gl::GlError),
}

impl Display for PlatformError {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(feature = "opengl")]
            PlatformError::GlError(e) => e.fmt(fmt),
            PlatformError::Handler(e) => e.fmt(fmt),
            PlatformError::InvalidWindowContext => {
                write!(fmt, "Invalid operation: WindowContext is not valid anymore")
            }
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformError::Handler(e) => Some(e.source()),
            _ => None,
        }
    }
}

impl From<HandlerError> for PlatformError {
    fn from(e: HandlerError) -> Self {
        PlatformError::Handler(e)
    }
}
