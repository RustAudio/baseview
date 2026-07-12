use crate::HandlerError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Win32(windows_core::Error),
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
