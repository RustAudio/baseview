use crate::win::handle::WindowHandleTransmitter;
use crate::win::win32_window::Win32Window;
use crate::win::Window;
use crate::{
    Event, MouseButton, MouseEvent, PhyPoint, PhySize, ScrollDelta, WindowEvent, WindowHandler,
};

use crate::win::drop_target::DropTarget;
use crate::win::keyboard::KeyboardState;
use std::cell::{Cell, RefCell, RefMut};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::ole2::RevokeDragDrop;
use winapi::um::winuser::{
    DefWindowProcW, DestroyWindow, GetWindowLongPtrW, PostMessageW, ReleaseCapture, SetCapture,
    SetWindowLongPtrW, TrackMouseEvent, GET_XBUTTON_WPARAM, GWLP_USERDATA, TRACKMOUSEEVENT,
    WHEEL_DELTA, WM_CHAR, WM_CLOSE, WM_CREATE, WM_DPICHANGED, WM_INPUTLANGCHANGE, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SHOWWINDOW, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_XBUTTONDOWN,
    WM_XBUTTONUP, XBUTTON1, XBUTTON2,
};

pub(crate) struct ProcState {
    pub(crate) window: Rc<Window>,
    // FIXME: do not expose this, expose handle_event/frame methods instead to ensure
    // the borrows aren't kept for too long in callers
    pub(crate) handler: RefCell<Box<dyn WindowHandler>>,
    handle_transmitter: WindowHandleTransmitter,
    _drop_target: Rc<DropTarget>,

    // Internals for proc event handling
    // TODO: refactor KeyboardState to use interior mutability
    pub(crate) keyboard_state: RefCell<KeyboardState>,
    mouse_button_counter: Cell<usize>,
    mouse_was_outside_window: Cell<bool>,
}

impl ProcState {
    pub fn new(
        window: Rc<Window>, handle_transmitter: WindowHandleTransmitter,
        handler: impl WindowHandler,
    ) -> Rc<Self> {
        Rc::new_cyclic(move |proc_state| Self {
            _drop_target: DropTarget::register(proc_state.clone(), &window.win32_window),
            window,
            handler: RefCell::new(Box::new(handler)),
            handle_transmitter,
            keyboard_state: RefCell::new(KeyboardState::new()),
            mouse_button_counter: Cell::new(0),
            mouse_was_outside_window: Cell::new(true),
        })
    }

    pub fn move_to_proc(self: Rc<Self>) {
        let handle = self.window.win32_window.handle();
        let proc_data_ptr = Rc::into_raw(self);

        unsafe {
            SetWindowLongPtrW(handle, GWLP_USERDATA, proc_data_ptr as _);
        }
    }

    pub fn handler_mut(&self) -> RefMut<Box<dyn WindowHandler>> {
        self.handler.borrow_mut()
    }

    unsafe fn destroy(ptr: *const Self) {
        {
            let state = &*ptr;
            state.handle_transmitter.notify_closed();

            let handle = state.window.win32_window.handle();
            RevokeDragDrop(handle);
            SetWindowLongPtrW(handle, GWLP_USERDATA, 0);
        }

        drop(Rc::from_raw(ptr));
    }
}

pub(crate) unsafe extern "system" fn wnd_proc(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        PostMessageW(hwnd, WM_SHOWWINDOW, 0, 0);
        return 0;
    }

    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ProcState;
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let result = catch_unwind(AssertUnwindSafe(move || {
        let state = &*state_ptr;
        let result = wnd_proc_inner(hwnd, msg, wparam, lparam, state);

        // If any of the above event handlers caused tasks to be pushed to the deferred tasks list,
        // then we'll try to handle them now
        state.window.handle_deferred_tasks();

        // NOTE: This is not handled in `wnd_proc_inner` because of the deferred task loop above
        if msg == WM_NCDESTROY {
            ProcState::destroy(state_ptr)
        }

        result
    }));

    // The actual custom window proc has been moved to another function so we can always handle
    // the deferred tasks regardless of whether the custom window proc returns early or not
    match result {
        Ok(Some(result)) => result,
        Ok(None) => DefWindowProcW(hwnd, msg, wparam, lparam),
        Err(_) => 0, // TODO: handle panics?
    }
}

/// Our custom `wnd_proc` handler. If the result contains a value, then this is returned after
/// handling any deferred tasks. otherwise the default window procedure is invoked.
unsafe fn wnd_proc_inner(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM, state: &ProcState,
) -> Option<LRESULT> {
    match msg {
        WM_MOUSEMOVE => {
            // FIXME: use TrackMouseEvent to generate the WM_MOUSEHOVER events instead of this
            if state.mouse_was_outside_window.get() {
                // this makes Windows track whether the mouse leaves the window.
                // When the mouse leaves it results in a `WM_MOUSELEAVE` event.
                let mut track_mouse = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: winapi::um::winuser::TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: winapi::um::winuser::HOVER_DEFAULT,
                };
                // Couldn't find a good way to track whether the mouse enters,
                // but if `WM_MOUSEMOVE` happens, the mouse must have entered.
                TrackMouseEvent(&mut track_mouse);
                state.mouse_was_outside_window.set(false);

                let enter_event = Event::Mouse(MouseEvent::CursorEntered);
                state.handler.borrow_mut().on_event(enter_event);
            }

            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let physical_pos = PhyPoint { x, y };
            let logical_pos = physical_pos.to_logical(&state.window.win32_window.current_size());
            let move_event = Event::Mouse(MouseEvent::CursorMoved {
                position: logical_pos,
                modifiers: state.keyboard_state.borrow().get_modifiers_from_mouse_wparam(wparam),
            });
            state.handler.borrow_mut().on_event(move_event);
            Some(0)
        }

        WM_MOUSELEAVE => {
            let event = Event::Mouse(MouseEvent::CursorLeft);
            state.handler.borrow_mut().on_event(event);

            state.mouse_was_outside_window.set(true);
            Some(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / WHEEL_DELTA as f32;

            let event = Event::Mouse(MouseEvent::WheelScrolled {
                delta: if msg == WM_MOUSEWHEEL {
                    ScrollDelta::Lines { x: 0.0, y: value }
                } else {
                    ScrollDelta::Lines { x: value, y: 0.0 }
                },
                modifiers: state.keyboard_state.borrow().get_modifiers_from_mouse_wparam(wparam),
            });

            state.handler.borrow_mut().on_event(event);

            Some(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_RBUTTONDOWN
        | WM_RBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let mut mouse_button_counter = state.mouse_button_counter.get();

            let button = match msg {
                WM_LBUTTONDOWN | WM_LBUTTONUP => Some(MouseButton::Left),
                WM_MBUTTONDOWN | WM_MBUTTONUP => Some(MouseButton::Middle),
                WM_RBUTTONDOWN | WM_RBUTTONUP => Some(MouseButton::Right),
                WM_XBUTTONDOWN | WM_XBUTTONUP => match GET_XBUTTON_WPARAM(wparam) {
                    XBUTTON1 => Some(MouseButton::Back),
                    XBUTTON2 => Some(MouseButton::Forward),
                    _ => None,
                },
                _ => None,
            };

            if let Some(button) = button {
                let event = match msg {
                    WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN | WM_XBUTTONDOWN => {
                        // Capture the mouse cursor on button down
                        mouse_button_counter = mouse_button_counter.saturating_add(1);
                        SetCapture(hwnd);
                        MouseEvent::ButtonPressed {
                            button,
                            modifiers: state
                                .keyboard_state
                                .borrow()
                                .get_modifiers_from_mouse_wparam(wparam),
                        }
                    }
                    WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP | WM_XBUTTONUP => {
                        // Release the mouse cursor capture when all buttons are released
                        mouse_button_counter = mouse_button_counter.saturating_sub(1);
                        if mouse_button_counter == 0 {
                            ReleaseCapture();
                        }

                        MouseEvent::ButtonReleased {
                            button,
                            modifiers: state
                                .keyboard_state
                                .borrow()
                                .get_modifiers_from_mouse_wparam(wparam),
                        }
                    }
                    _ => {
                        unreachable!()
                    }
                };

                state.mouse_button_counter.set(mouse_button_counter);

                state.handler.borrow_mut().on_event(Event::Mouse(event));
            }

            None
        }
        WM_TIMER => {
            if wparam == Win32Window::WIN_FRAME_TIMER {
                state.handler.borrow_mut().on_frame();
            }

            Some(0)
        }
        WM_CLOSE => {
            // Make sure to release the borrow before the DefWindowProc call
            {
                state.handler.borrow_mut().on_event(Event::Window(WindowEvent::WillClose));
            }

            // DestroyWindow(hwnd);
            // Some(0)
            Some(DefWindowProcW(hwnd, msg, wparam, lparam))
        }
        WM_CHAR | WM_SYSCHAR | WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
        | WM_INPUTLANGCHANGE => {
            let opt_event =
                state.keyboard_state.borrow_mut().process_message(hwnd, msg, wparam, lparam);

            if let Some(event) = opt_event {
                state.handler.borrow_mut().on_event(Event::Keyboard(event));
            }

            if msg != WM_SYSKEYDOWN {
                Some(0)
            } else {
                None
            }
        }
        WM_SIZE => {
            let new_size = PhySize {
                width: (lparam & 0xFFFF) as u16 as u32,
                height: ((lparam >> 16) & 0xFFFF) as u16 as u32,
            };

            // Only send the event if anything changed
            if let Some(new_window_info) = state.window.win32_window.resized(new_size) {
                state
                    .handler
                    .borrow_mut()
                    .on_event(Event::Window(WindowEvent::Resized(new_window_info)));
            };

            None
        }
        WM_DPICHANGED => {
            let dpi = (wparam & 0xFFFF) as u16 as u32;
            let scale_factor = dpi as f64 / 96.0;

            state.window.win32_window.update_scale_factor(scale_factor);

            None
        }
        // NOTE: `WM_NCDESTROY` is handled in the outer function because this deallocates the window
        //        state
        Win32Window::BV_WINDOW_MUST_CLOSE => {
            DestroyWindow(hwnd);
            Some(0)
        }
        _ => None,
    }
}
