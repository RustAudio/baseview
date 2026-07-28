use super::*;
use crate::platform::Result;

pub trait WindowHandler: 'static {
    /// Requests the handler to draw a new frame.
    ///
    /// If this returns an error, the window will be considered unable to render its contents, and
    /// will be subsequently closed.
    fn on_frame(&self) -> core::result::Result<(), HandlerError>;
    /// Informs the handler that the window has been resized.
    ///
    /// # Errors
    ///
    /// This operation can fail, in which case an [`HandlerError`] can be returned.
    /// This can happen if e.g. an underlying buffer could not be resized, or some kind of driver error.
    ///
    /// In case this `resized` operation fails, `baseview` will assume that it did not meaningfully
    /// change anything, and that the window is still able to render and operate at the previous size.
    ///
    /// It will also attempt to resize the underlying platform window and parent window back to the
    /// previous size, but this is only a best-effort attempt since those operations can also fail.
    fn resized(&self, new_size: WindowSize) -> core::result::Result<(), HandlerError>;
    fn on_event(&self, event: Event) -> EventStatus;
}

type DynBuilderResult = core::result::Result<Box<dyn WindowHandler>, HandlerError>;

#[allow(unused)]
pub struct WindowHandlerBuilder {
    inner: Box<dyn FnOnce(WindowContext) -> DynBuilderResult + Send + 'static>,
}

impl WindowHandlerBuilder {
    pub fn new<H: WindowHandler>(
        f: impl FnOnce(WindowContext) -> core::result::Result<H, HandlerError> + Send + 'static,
    ) -> WindowHandlerBuilder {
        Self { inner: Box::new(|c| Ok(Box::new(f(c)?))) }
    }

    pub fn build(self, ctx: WindowContext) -> Result<Box<dyn WindowHandler>> {
        match (self.inner)(ctx) {
            Ok(handle) => Ok(handle),
            Err(e) => Err(platform::Error::Handler(e)),
        }
    }
}
