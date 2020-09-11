use baseview::{Event, Window, WindowHandler};

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview",
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
    };

    let _handle = Window::open::<MyProgram>(window_open_options);
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
            Event::Interval(delta_time) => println!("Update interval, delta time: {}", delta_time),
            Event::Mouse(e) => println!("Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
            Event::Window(e) => println!("Window event: {:?}", e),
            Event::FileDrop(e) => println!("File drop event: {:?}", e),
        }
    }

    fn on_message(&mut self, window: &mut Window, _message: Self::Message) {}
}
