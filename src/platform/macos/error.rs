use crate::HandlerError;
use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    Handler(HandlerError),
    #[cfg(feature = "opengl")]
    GlError(super::gl::GlError),
}

impl Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(feature = "opengl")]
            Error::GlError(e) => e.fmt(fmt),
            Error::Handler(e) => e.fmt(fmt),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Handler(e) => Some(e),
            _ => None,
        }
    }
}
