use crate::{Event, EventStatus};

pub trait WindowHandler: 'static {
    fn on_frame(&self);
    fn on_event(&self, event: Event) -> EventStatus;
}
