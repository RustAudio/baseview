use crate::context::WindowContext;
use crate::{Event, EventStatus, Window};
use std::cell::{Cell, OnceCell};

pub trait WindowHandler<'a> {
    fn on_frame(&mut self, window: &mut Window);
    fn on_event(&mut self, window: &mut Window, event: Event) -> EventStatus;
}

pub(crate) struct HandlerContainer {
    initializer:
        Cell<Option<Box<dyn for<'a> FnOnce(WindowContext<'a>) -> Box<dyn WindowHandler<'a>>>>>,
    handler: OnceCell<Box<dyn WindowHandler<'static>>>,
}

impl HandlerContainer {
    pub fn new<H: for<'a> WindowHandler<'a>>(
        initializer: impl for<'a> FnOnce(WindowContext<'a>) -> H,
    ) -> Self {
        Self {
            initializer: Cell::new(Some(Box::new(|w| Box::new(initializer(w))))),
            handler: OnceCell::new(),
        }
    }

    pub fn initialize(&self, context: WindowContext<'static>) -> &dyn WindowHandler {
        let initializer = self.initializer.take().unwrap();
        let result = initializer(context);
        self.handler.set(result).unwrap();
        self.handler.get().unwrap()
    }

    pub fn get(&self) -> Option<&dyn WindowHandler> {
        self.handler.get()
    }
}
