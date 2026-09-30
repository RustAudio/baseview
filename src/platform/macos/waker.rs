use crate::platform::macos::view::BaseviewView;
use crate::wrappers::appkit::{MainThreadBoundWeak, View};
use objc2::rc::Weak;
use std::time::Duration;

#[derive(Clone)]
pub struct WindowWaker {
    view: MainThreadBoundWeak<View<BaseviewView>>,
}

impl WindowWaker {
    pub fn new(reference: Weak<View<BaseviewView>>) -> Self {
        Self { view: MainThreadBoundWeak::new(reference) }
    }

    pub fn request_redraw(&self) {
        self.request_redraw_after(Duration::ZERO)
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        self.view.use_on_main_thread_after(duration, |view| {
            let Some(view) = view.load() else { return };
            let Some(view) = view.inner() else { return };
            view.set_next_frame_needed(true);
        })
    }

    pub fn request_poll(&self) {
        self.view.use_on_main_thread(|view| {
            let Some(view) = view.load() else { return };
            let Some(view) = view.inner_ref() else { return };

            BaseviewView::poll(view);
        })
    }
}
