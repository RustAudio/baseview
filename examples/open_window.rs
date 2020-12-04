use std::time::Duration;

use baseview::{Event, Window, WindowHandler, WindowScalePolicy};


#[derive(Debug, Clone)]
enum Message {
    Hello
}


struct OpenWindowExample;


impl WindowHandler for OpenWindowExample {
    type Message = Message;

    fn on_frame(&mut self) {}

    fn on_event(&mut self, _window: &mut Window, event: Event) {
        match event {
            Event::Mouse(e) => println!("Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
            Event::Window(e) => println!("Window event: {:?}", e),
        }
    }

    fn on_message(&mut self, _window: &mut Window, message: Self::Message) {
        println!("Message: {:?}", message);
    }
}


fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview".into(),
        size: baseview::Size::new(512.0, 512.0),
        scale: WindowScalePolicy::SystemScaleFactor,
        parent: baseview::Parent::None,
    };

    let (mut handle, opt_app_runner) = Window::open(
        window_open_options,
        |_| OpenWindowExample
    );

    ::std::thread::spawn(move || {
        loop {
            ::std::thread::sleep(Duration::from_secs(5));

            if let Err(_) = handle.try_send_message(Message::Hello){
                println!("Failed sending message");
            }
        }
    });

    opt_app_runner.unwrap().app_run_blocking();
}
