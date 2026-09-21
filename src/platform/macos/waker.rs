use crate::platform::macos::view::BaseviewView;
use crate::wrappers::appkit::View;
use dispatch2::MainThreadBound;
use objc2::rc::Weak;
use objc2::MainThreadMarker;
use std::time::Duration;

pub struct WindowWaker {
    view: MainThreadBound<Weak<View<BaseviewView>>>,
}

impl Clone for WindowWaker {
    fn clone(&self) -> Self {
        // SAFETY: we only use this to clone the inner Weak handle, which is always thread-safe.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };

        Self { view: MainThreadBound::new(Weak::clone(self.view.get(mtm)), mtm) }
    }
}

impl WindowWaker {
    pub fn request_redraw(&self) {
        self.request_redraw_after(Duration::ZERO)
    }

    pub fn request_redraw_after(&self, duration: Duration) {
        self.view.get_on_main(|view| {
            let Some(view) = view.load() else { return };
            if duration.is_zero() {
                view.setNeedsDisplay(true)
            } else {
                todo!()
            }
        })
    }

    pub fn request_poll(&self) {
        self.view.get_on_main(|view| {
            let Some(view) = view.load() else { return };
            let Some(view) = view.inner_ref() else { return };

            BaseviewView::poll(view);
        })
    }
}
