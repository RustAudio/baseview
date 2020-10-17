use baseview::{Event, Window, WindowHandler, WindowSize, WindowScalePolicy};

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
        size: WindowSize::MinMaxLogical {
            initial_size: baseview::Size::new(512.0, 512.0),
            min_size: baseview::Size::new(200.0, 200.0),
            max_size: baseview::Size::new(1024.0, 1024.0),
            keep_aspect: false,
        },
        scale: WindowScalePolicy::TrySystemScaleFactor,
        parent: baseview::Parent::None,
    };

    let (handle, _window_info) = Window::open::<MyProgram>(window_open_options).unwrap();
    handle.app_run_blocking();
}
