use crate::{HostHandler, WindowContext, WindowHandle, WindowHandler};
use dpi::{LogicalSize, Size};

#[non_exhaustive]
pub struct WindowBuilder {
    pub title: Option<String>,
    pub size: Size,
    pub host_handler: Option<Box<dyn HostHandler>>,
    pub parented: bool,
}

impl WindowBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn parented(mut self) -> Self {
        self.parented = true;
        self
    }

    pub fn hosted(mut self, handler: impl HostHandler) -> Self {
        self.host_handler = Some(Box::new(handler));
        self
    }

    pub fn build<H: WindowHandler>(
        self, handler_builder: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) -> WindowHandle {
        todo!()
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self {
            title: None,
            size: LogicalSize::new(420.0, 240.0).into(),
            host_handler: None,
            parented: false,
        }
    }
}
