use std::sync::mpsc;

use baseview::{Event, WindowState};

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
        frame_rate: 60.0,
    };

    let (_app_message_tx, app_message_rx) = mpsc::channel::<()>();

    // Send _app_message_tx to a separate thread, then send messages to the GUI thread.

    let _ = baseview::Window::<MyProgram>::open(window_open_options, app_message_rx);
}
struct MyProgram {}

impl baseview::AppWindow for MyProgram {
    type AppMessage = ();

    fn build(window: &mut WindowState) -> Self {
        println!("Window info: {:?}", window);
        Self {}
    }

    fn draw(&mut self) {}

    fn on_event(&mut self, event: Event, _window: &mut WindowState) {
        match event {
            Event::Interval(delta_time) => println!("Update interval, delta time: {}", delta_time),
            Event::Mouse(e) => println!("Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
            Event::Window(e) => println!("Window event: {:?}", e),
            Event::FileDrop(e) => println!("File drop event: {:?}", e),
            Event::Clipboard(maybe_string) => {
                if let Some(string) = maybe_string {
                    println!("Clipboard: {}", string);
                } else {
                    println!("Clipboard cleared");
                }
            }
        }
    }

    fn on_app_message(&mut self, _message: Self::AppMessage, _window: &mut WindowState) {}
}
