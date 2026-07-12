use std::fmt::{Debug, Display, Formatter};

pub type Result<T> = std::result::Result<T, Error>;

pub struct Error {
    inner: crate::platform::Error,
}

impl From<crate::platform::Error> for Error {
    fn from(inner: crate::platform::Error) -> Error {
        Error { inner }
    }
}

impl Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl std::error::Error for Error {}

pub struct HandlerError {
    inner: Box<dyn std::error::Error + 'static>,
}

impl HandlerError {
    #[inline]
    pub fn cause(&self) -> &(dyn std::error::Error + 'static) {
        self.inner.as_ref()
    }

    #[inline]
    pub fn into_inner(self) -> Box<dyn std::error::Error + 'static> {
        self.inner
    }
}

impl Debug for HandlerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for HandlerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl<E: std::error::Error + 'static> From<E> for HandlerError {
    fn from(value: E) -> Self {
        Self { inner: Box::new(value) }
    }
}
