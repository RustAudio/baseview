use baseview::{Event, Window, WindowHandler};

struct MyProgram {}

impl WindowHandler for MyProgram {
    type Message = ();

    fn build(_window: &mut Window) -> Self {
        Self {}
    }

    fn on_frame(&mut self) {}

    fn on_event(&mut self, _window: &mut Window, event: Event) {
        match event {
            Event::Mouse(e) => println!("Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
            Event::Window(e) => println!("Window event: {:?}", e),
        }
    }

    fn on_message(&mut self, _window: &mut Window, _message: Self::Message) {}
}

fn main() {
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview".into(),
        width: 512,
        height: 512,
        parent: baseview::Parent::None,
        ..Default::default()
    };

    let handle = Window::open::<MyProgram>(window_open_options);
    handle.app_run_blocking();
}
