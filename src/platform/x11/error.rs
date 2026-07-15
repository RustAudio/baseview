use crate::platform::x11::drag_n_drop::ParseError;
use crate::platform::x11::xcb_connection::GetPropertyError;
use crate::warn;
use crate::wrappers::xlib::{DisplayOpenFailedError, InitThreadsFailedError};
use crate::HandlerError;
use std::sync::mpsc::RecvError;
use x11_dl::error::OpenError;
use x11rb::connection::RequestConnection;
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectError, ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::x11_utils::{TryParse, X11Error};

#[derive(Debug)]
pub enum Error {
    CreationFailed(String),
    Io(std::io::Error),
    DylibOpen(OpenError),
    InitThreadsFailed(InitThreadsFailedError),
    X11(X11Error),
    Connection(ConnectionError),
    IdsExhausted,
    Parse(ParseError),
    GetProperty(GetPropertyError),
    Connect(ConnectError),
    DisplayOpenFailed(DisplayOpenFailedError),
    Handler(HandlerError),
    Channel(RecvError),
    #[cfg(feature = "opengl")]
    XLib(crate::wrappers::xlib::XLibError),
    #[cfg(feature = "opengl")]
    Gl(super::gl::CreationFailedError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Self::IdsExhausted => f.write_str("X11 IDs have been exhausted"),
            _ => todo!(),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<OpenError> for Error {
    fn from(value: OpenError) -> Self {
        Self::DylibOpen(value)
    }
}

impl From<InitThreadsFailedError> for Error {
    fn from(value: InitThreadsFailedError) -> Self {
        Self::InitThreadsFailed(value)
    }
}

impl From<DisplayOpenFailedError> for Error {
    fn from(value: DisplayOpenFailedError) -> Self {
        Self::DisplayOpenFailed(value)
    }
}

impl From<ConnectionError> for Error {
    fn from(value: ConnectionError) -> Self {
        Self::Connection(value)
    }
}

impl From<X11Error> for Error {
    fn from(value: X11Error) -> Self {
        Self::X11(value)
    }
}

impl From<HandlerError> for Error {
    fn from(value: HandlerError) -> Self {
        Self::Handler(value)
    }
}

#[cfg(feature = "opengl")]
impl From<crate::wrappers::xlib::XLibError> for Error {
    fn from(value: crate::wrappers::xlib::XLibError) -> Self {
        Self::XLib(value)
    }
}

impl From<ParseError> for Error {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<GetPropertyError> for Error {
    fn from(value: GetPropertyError) -> Self {
        Self::GetProperty(value)
    }
}

impl From<ConnectError> for Error {
    fn from(value: ConnectError) -> Self {
        Self::Connect(value)
    }
}

// X11rb aggregate error types

impl From<ReplyOrIdError> for Error {
    fn from(value: ReplyOrIdError) -> Self {
        match value {
            ReplyOrIdError::IdsExhausted => Self::IdsExhausted,
            ReplyOrIdError::ConnectionError(e) => Self::Connection(e),
            ReplyOrIdError::X11Error(e) => Self::X11(e),
        }
    }
}

impl From<ReplyError> for Error {
    fn from(value: ReplyError) -> Self {
        match value {
            ReplyError::ConnectionError(e) => Self::Connection(e),
            ReplyError::X11Error(e) => Self::X11(e),
        }
    }
}

#[cfg(feature = "opengl")]
impl From<super::gl::CreationFailedError> for Error {
    fn from(value: super::gl::CreationFailedError) -> Self {
        Self::Gl(value)
    }
}

pub trait CookieExt {
    fn check_warn(self);
}

impl<T: RequestConnection> CookieExt for VoidCookie<'_, T> {
    fn check_warn(self) {
        if let Err(e) = self.check() {
            warn!(e);
        }
    }
}

pub trait ReplyExt<R> {
    fn reply_or_warn(self) -> Option<R>;
}

impl<R: TryParse, C: RequestConnection> ReplyExt<R> for Cookie<'_, C, R> {
    fn reply_or_warn(self) -> Option<R> {
        match self.reply() {
            Ok(r) => Some(r),
            Err(e) => {
                warn!(e);
                None
            }
        }
    }
}
