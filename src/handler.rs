use crate::{Event, EventStatus};

pub trait WindowHandler: 'static {
    fn on_frame(&mut self);
    fn on_event(&mut self, event: Event) -> EventStatus;
}
