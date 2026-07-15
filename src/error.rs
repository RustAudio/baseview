use std::fmt::{Debug, Display, Formatter};

/// An error that can occur during window creation or manipulation.
///
/// This error type is opaque: you can only get error information from its [`Display`] implementation.
///
/// Which errors can occur on which operations can greatly vary between platforms. For instance on X11,
/// the connection to the server can be lost, or the X server can send an invalid message, which is
/// not possible on e.g. Windows or macOS.
///
/// This is the general Baseview error type.
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

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl From<HandlerError> for Error {
    fn from(e: HandlerError) -> Self {
        Self { inner: crate::platform::Error::Handler(e) }
    }
}

/// An error that can be returned from a [`WindowHandler`](crate::WindowHandler).
///
/// This type does not implement the [`Error`] trait: instead it can be created from any kind of
/// [`Error`] type, using its [`From`] implementation and/or the `?` operator.
///
/// [`Error`]: std::error::Error
pub struct HandlerError {
    inner: Box<dyn std::error::Error + 'static>,
}

impl HandlerError {
    #[inline]
    pub fn source(&self) -> &(dyn std::error::Error + 'static) {
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
