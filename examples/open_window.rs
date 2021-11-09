use std::time::Duration;

use rtrb::{Consumer, RingBuffer};

use baseview::{Event, EventStatus, Window, WindowHandler, WindowScalePolicy};

#[derive(Debug, Clone)]
enum Message {
    Hello,
}

struct OpenWindowExample {
    rx: Consumer<Message>,
}

impl WindowHandler for OpenWindowExample {
    fn on_frame(&mut self, _window: &mut Window) {
        while let Ok(message) = self.rx.pop() {
            println!("Message: {:?}", message);
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Mouse(e) => println!("Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
            Event::Window(e) => println!("Window event: {:?}", e),
        }

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview".into(),
        size: baseview::Size::new(512.0, 512.0),
        scale: WindowScalePolicy::SystemScaleFactor,
    };

    let (mut tx, rx) = RingBuffer::new(128).split();

    ::std::thread::spawn(move || loop {
        ::std::thread::sleep(Duration::from_secs(5));

        if let Err(_) = tx.push(Message::Hello) {
            println!("Failed sending message");
        }
    });

    Window::open_blocking(window_open_options, |_| OpenWindowExample { rx });
}
