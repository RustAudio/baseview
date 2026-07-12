use super::*;
use crate::platform::Result;

pub trait WindowHandler: 'static {
    fn on_frame(&self);
    fn resized(&self, new_size: WindowSize);
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
