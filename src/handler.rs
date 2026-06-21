use crate::{Event, EventStatus, WindowSize};

pub trait WindowHandler: 'static {
    fn on_frame(&self);
    fn resized(&self, new_size: WindowSize);
    fn on_event(&self, event: Event) -> EventStatus;
}
