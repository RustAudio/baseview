use crate::platform::x11::error::FatalError;
use crate::{DamageArea, EventStatus, WindowHandler, WindowSize};

pub struct Handler {
    handler: Box<dyn WindowHandler>,
}

impl Handler {
    pub fn new(handler: Box<dyn WindowHandler>) -> Self {
        Self { handler }
    }

    pub fn draw(&self) -> Result<(), FatalError> {
        self.handler.draw().map_err(|e| FatalError::Redraw(e.to_string()))
    }

    pub fn resize(&self, new_size: WindowSize) -> Result<(), ()> {
        if let Err(e) = self.handler.resized(new_size) {
            crate::warn!("Failed to resize window: {}", e);
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn poll(&self) {
        self.handler.poll()
    }

    pub fn on_event(&self, event: crate::Event) -> EventStatus {
        self.handler.on_event(event)
    }

    pub fn damage(&self, area: DamageArea) {
        self.handler.damage(area)
    }
}
