use baseview::Message;

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
    };

    let my_program = MyProgram {};

    let _ = baseview::Window::open(window_open_options, my_program);
}
struct MyProgram {}

impl baseview::Receiver for MyProgram {
    fn create_context(
        &mut self,
        _window: raw_window_handle::RawWindowHandle,
        _window_info: &baseview::WindowInfo,
    ) {
    }

    fn on_message(&mut self, message: Message) {
        match message {
            Message::RenderExpose => {}
            Message::CursorMotion(x, y) => {
                println!("Cursor moved, x: {}, y: {}", x, y);
            }
            Message::MouseDown(button_id) => {
                println!("Mouse down, button id: {:?}", button_id);
            }
            Message::MouseUp(button_id) => {
                println!("Mouse up, button id: {:?}", button_id);
            }
            Message::MouseScroll(mouse_scroll) => {
                println!("Mouse scroll, {:?}", mouse_scroll);
            }
            Message::MouseClick(mouse_click) => {
                println!("Mouse click, {:?}", mouse_click);
            }
            Message::KeyDown(keycode) => {
                println!("Key down, keycode: {}", keycode);
            }
            Message::KeyUp(keycode) => {
                println!("Key up, keycode: {}", keycode);
            }
            Message::CharacterInput(char_code) => {
                println!("Character input, char_code: {}", char_code);
            }
            Message::WindowResized(window_info) => {
                println!("Window resized, {:?}", window_info);
            }
            Message::WindowFocus => {
                println!("Window focused");
            }
            Message::WindowUnfocus => {
                println!("Window unfocused");
            }
            Message::WillClose => {
                println!("Window will close");
            }
        }
    }
}
