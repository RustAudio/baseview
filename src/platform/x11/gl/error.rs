use super::*;
use crate::wrappers::xlib::XLibError;

use std::fmt::{Display, Formatter};
use std::num::TryFromIntError;
use x11_dl::error::OpenError;
use x11rb::protocol::xproto::Visualid;

#[derive(Debug)]
pub enum GlCreationFailedError {
    NoValidFBConfig,
    NoVisual,
    GetProcAddressFailed,
    MakeCurrentFailed,
    ContextCreationFailed,
    X11Error(XLibError),
    OpenError(OpenError),
    EGLLoadError(libloading::Error),
    EGLMissingSymbol(MissingSymbolError),
    Egl(EglError),
    EglNoDisplay,
    EglUnsupportedVersion(EglVersion),
    EglUnknownVisualId(Visualid),
    EglInvalidVisualId(i32, TryFromIntError),
}

use GlCreationFailedError::*;

impl Display for GlCreationFailedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NoValidFBConfig => f.write_str("Could not find a valid Framebuffer configuration"),
            NoVisual => f.write_str("Could not find a matching visual configuration"),
            GetProcAddressFailed => f.write_str("GetProcAddress failed"),
            MakeCurrentFailed => f.write_str("MakeCurrent failed"),
            ContextCreationFailed => f.write_str("Failed to create GL context"),
            X11Error(e) => e.fmt(f),
            OpenError(e) => e.fmt(f),
            EGLLoadError(e) => {
                write!(f, "Could not load EGL library: {e}, {:?}", e.source())
            }
            EGLMissingSymbol(e) => e.fmt(f),
            Egl(e) => e.fmt(f),
            EglNoDisplay => f.write_str("EGL returned no valid display"),
            EglUnsupportedVersion(e) => {
                write!(f, "Unsupported EGL version: {}.{} (EGL 1.5 is required)", e.major, e.minor)
            }
            EglInvalidVisualId(id, e) => {
                write!(f, "Invalid Visual ID ({id}) returned by EGL: {e}")
            }
            EglUnknownVisualId(id) => {
                write!(f, "Unknown Visual ID returned by EGL: {id}")
            }
        }
    }
}

impl From<EglError> for GlCreationFailedError {
    fn from(err: EglError) -> Self {
        Egl(err)
    }
}
