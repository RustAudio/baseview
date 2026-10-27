use crate::platform::BaseviewWindow;
use crate::wrappers::win32::window::{HWnd, WmMouse};
use crate::{Event, MouseButton, MouseEvent, ScrollDelta};
use dpi::PhysicalPosition;
use windows_sys::Win32::Foundation::WPARAM;

impl WmMouse for BaseviewWindow {
    fn on_move(&self, window: HWnd, position: PhysicalPosition<i16>, wparam: WPARAM) {
        if self.window_state.mouse_was_outside_window.get() {
            // this makes Windows track whether the mouse leaves the window.
            // When the mouse leaves it results in a `WM_MOUSELEAVE` event.
            // Couldn't find a good way to track whether the mouse enters,
            // but if `WM_MOUSEMOVE` happens, the mouse must have entered.
            let _ = window.start_cursor_leave_tracking();
            self.window_state.mouse_was_outside_window.set(false);

            self.handle_event(Event::Mouse(MouseEvent::CursorEntered));
        }

        let move_event = Event::Mouse(MouseEvent::CursorMoved {
            position: position.cast(),
            modifiers: self
                .window_state
                .keyboard_state
                .borrow()
                .get_modifiers_from_mouse_wparam(wparam),
        });

        self.handle_event(move_event);
    }

    fn on_leave(&self, _: HWnd) {
        self.window_state.mouse_was_outside_window.set(true);
        self.handle_event(Event::Mouse(MouseEvent::CursorLeft));
    }

    fn on_wheel(&self, _: HWnd, delta: ScrollDelta, wparam: WPARAM) {
        let event = Event::Mouse(MouseEvent::WheelScrolled {
            delta,
            modifiers: self
                .window_state
                .keyboard_state
                .borrow()
                .get_modifiers_from_mouse_wparam(wparam),
        });

        self.handle_event(event);
    }

    fn on_capture_changed(&self, window: HWnd, window_gaining_capture: Option<HWnd>) {
        if window_gaining_capture != Some(window) {
            let previous_counter = self.window_state.mouse_button_counter.replace(0);
            HWnd::release_capture();

            if previous_counter == 0 {
                self.handle_event(Event::Mouse(MouseEvent::CursorLeft));
                self.window_state.mouse_was_outside_window.set(true);
            }
        }
    }

    fn on_button(&self, window: HWnd, button: MouseButton, down: bool, wparam: WPARAM) {
        let mut mouse_button_counter = self.window_state.mouse_button_counter.get();
        let modifiers =
            self.window_state.keyboard_state.borrow().get_modifiers_from_mouse_wparam(wparam);

        let event = if down {
            // Capture the mouse cursor on button down
            mouse_button_counter = mouse_button_counter.saturating_add(1);
            window.set_capture();

            MouseEvent::ButtonPressed { button, modifiers }
        } else {
            mouse_button_counter = mouse_button_counter.saturating_sub(1);

            // Release the mouse cursor capture when all buttons are released
            if mouse_button_counter == 0 {
                HWnd::release_capture();
            }

            MouseEvent::ButtonReleased { button, modifiers }
        };

        self.window_state.mouse_button_counter.set(mouse_button_counter);
        self.handle_event(Event::Mouse(event));
    }
}
