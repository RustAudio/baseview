use std::sync::mpsc;

use baseview::Message;

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
    };

    let (message_tx, message_rx) = mpsc::channel::<Message>();

    let my_program = MyProgram {};

    let window = baseview::Window::build(window_open_options, message_tx);

    // Get raw window handle!
    let _raw_handle = window.raw_window_handle();

    my_program_loop(my_program, message_rx);
}
struct MyProgram {}

fn my_program_loop(_my_program: MyProgram, message_rx: mpsc::Receiver<Message>) {
    loop {
        let message = message_rx.recv().unwrap();
        match message {
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
            Message::Opened(window_info) => {
                println!("Window opened, {:?}", window_info);
            }
            Message::WillClose => {
                println!("Window will close");
            }
        }
    }
}
