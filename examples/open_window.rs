use std::time::Duration;

use rtrb::{RingBuffer, Consumer};

use baseview::{Event, Window, WindowHandler, WindowScalePolicy};

#[derive(Debug, Clone)]
enum Message {
    Hello
}

struct OpenWindowExample {
    rx: Consumer<Message>,
}

impl WindowHandler for OpenWindowExample {
    fn on_frame(&mut self) {
        while let Ok(message) = self.rx.pop() {
            println!("Message: {:?}", message);
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) {
        match event {
            Event::Mouse(e) => println!("Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
            Event::Window(e) => println!("Window event: {:?}", e),
        }
    }
}

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview".into(),
        size: baseview::Size::new(512.0, 512.0),
        scale: WindowScalePolicy::SystemScaleFactor,
        parent: baseview::Parent::None,
    };

    let (mut tx, rx) = RingBuffer::new(128).split();

    let opt_app_runner = Window::open(
        window_open_options,
        |_| OpenWindowExample { rx }
    );

    ::std::thread::spawn(move || {
        loop {
            ::std::thread::sleep(Duration::from_secs(5));

            if let Err(_) = tx.push(Message::Hello) {
                println!("Failed sending message");
            }
        }
    });

    opt_app_runner.unwrap().app_run_blocking();
}
