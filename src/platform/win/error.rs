use crate::HandlerError;
use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Win32(windows_core::Error),
    ResizeFailed,
    Handler(HandlerError),
}

impl From<windows_core::Error> for Error {
    fn from(value: windows_core::Error) -> Self {
        Error::Win32(value)
    }
}

impl From<HandlerError> for Error {
    fn from(value: HandlerError) -> Self {
        Error::Handler(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Win32(e) => Display::fmt(e, f),
            Error::Handler(e) => Display::fmt(e, f),
            Error::ResizeFailed => f.write_str("Window resize request failed."),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Win32(e) => Some(e),
            Error::Handler(e) => Some(e.source()),
            _ => None,
        }
    }
}
