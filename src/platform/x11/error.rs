use crate::platform::x11::drag_n_drop::ParseError;
use crate::platform::x11::window_thread::RequestFailed;
use crate::platform::x11::xcb_connection::GetPropertyError;
use crate::warn;
use crate::wrappers::xlib::{DisplayOpenFailedError, InitThreadsFailedError};
use crate::HandlerError;
use std::fmt::{Display, Formatter};
use x11_dl::error::OpenError;
use x11rb::connection::RequestConnection;
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectError, ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::x11_utils::{TryParse, X11Error};

#[derive(Debug)]
pub enum FatalError {
    Connection(ConnectionError),
    SendMainThread,
}

impl Display for FatalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FatalError::Connection(e) => e.fmt(f),
            FatalError::SendMainThread => {
                f.write_str("Failed to send callback from X11 thread to main thread")
            }
        }
    }
}

impl std::error::Error for FatalError {}

impl From<ConnectionError> for FatalError {
    fn from(err: ConnectionError) -> FatalError {
        FatalError::Connection(err)
    }
}

#[derive(Debug)]
pub enum Error {
    CreationFailed(String),
    Run(String),
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
    MainThreadRecvResult,
    Calloop(calloop::Error),
    RequestFromMainThreadFailed(RequestFailed),
    #[cfg(feature = "opengl")]
    XLib(crate::wrappers::xlib::XLibError),
    #[cfg(feature = "opengl")]
    Gl(super::gl::CreationFailedError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Self::IdsExhausted => f.write_str("X11 IDs have been exhausted"),
            Error::CreationFailed(e) => write!(f, "Failed to create window: {e}"),
            Error::Run(e) => write!(f, "Error in running X11 thread: {e}"),
            Error::DylibOpen(e) => e.fmt(f),
            Error::InitThreadsFailed(e) => e.fmt(f),
            Error::X11(e) => write!(f, "X server replied with error: {e:?}"),
            Error::Connection(e) => e.fmt(f),
            Error::Parse(e) => e.fmt(f),
            Error::GetProperty(e) => e.fmt(f),
            Error::Connect(e) => e.fmt(f),
            Error::DisplayOpenFailed(e) => e.fmt(f),
            Error::Handler(e) => e.fmt(f),
            Error::MainThreadRecvResult => {
                f.write_str("Failed to receive Window creation response from X11 thread: channel was closed unexpectedly")
            }
            Error::Calloop(e) => e.fmt(f),
            Error::RequestFromMainThreadFailed(e) => e.fmt(f),
            #[cfg(feature = "opengl")]
            Error::XLib(e) => e.fmt(f),
            #[cfg(feature = "opengl")]
            Error::Gl(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::DylibOpen(e) => Some(e),
            Error::Connect(e) => Some(e),
            Error::Handler(e) => Some(e.source()),
            #[cfg(feature = "opengl")]
            Error::XLib(e) => Some(e),
            _ => None,
        }
    }
}

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

impl From<calloop::Error> for Error {
    fn from(value: calloop::Error) -> Self {
        Self::Calloop(value)
    }
}

impl From<RequestFailed> for Error {
    fn from(value: RequestFailed) -> Self {
        Self::RequestFromMainThreadFailed(value)
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
            warn!("{}", e);
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
                warn!("{}", e);
                None
            }
        }
    }
}
