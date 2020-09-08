use baseview::{Event, Window, WindowHandler};

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
    };

    Window::open::<MyProgram>(window_open_options);
}

struct MyProgram {}

impl WindowHandler for MyProgram {
    type Message = ();

    fn build(window: &mut Window) -> Self {
        Self {}
    }

    fn draw(&mut self, window: &mut Window) {}

    fn on_event(&mut self, window: &mut Window, event: Event) {
        match event {
            Event::CursorMotion(x, y) => {
                println!("Cursor moved, x: {}, y: {}", x, y);
            }
            Event::MouseDown(button_id) => {
                println!("Mouse down, button id: {:?}", button_id);
            }
            Event::MouseUp(button_id) => {
                println!("Mouse up, button id: {:?}", button_id);
            }
            Event::MouseScroll(mouse_scroll) => {
                println!("Mouse scroll, {:?}", mouse_scroll);
            }
            Event::MouseClick(mouse_click) => {
                println!("Mouse click, {:?}", mouse_click);
            }
            Event::KeyDown(keycode) => {
                println!("Key down, keycode: {}", keycode);
            }
            Event::KeyUp(keycode) => {
                println!("Key up, keycode: {}", keycode);
            }
            Event::CharacterInput(char_code) => {
                println!("Character input, char_code: {}", char_code);
            }
            Event::WindowResized(window_info) => {
                println!("Window resized, {:?}", window_info);
            }
            Event::WindowFocus => {
                println!("Window focused");
            }
            Event::WindowUnfocus => {
                println!("Window unfocused");
            }
            Event::WillClose => {
                println!("Window will close");
            }
        }
    }

    fn on_message(&mut self, window: &mut Window, _message: Self::Message) {}
}
