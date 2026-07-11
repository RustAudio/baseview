use super::*;

pub trait WindowHandler: 'static {
    fn on_frame(&self);
    fn resized(&self, new_size: WindowSize);
    fn on_event(&self, event: Event) -> EventStatus;
}

#[allow(unused)]
pub struct WindowHandlerBuilder {
    inner: Box<dyn FnOnce(WindowContext) -> Box<dyn WindowHandler> + Send + 'static>,
}

impl WindowHandlerBuilder {
    pub fn new<H: WindowHandler>(
        f: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) -> WindowHandlerBuilder {
        Self { inner: Box::new(|c| Box::new(f(c))) }
    }

    pub fn build(self, ctx: WindowContext) -> Box<dyn WindowHandler> {
        (self.inner)(ctx)
    }
}
