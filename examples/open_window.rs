use baseview::{Event, Window, WindowHandler, WindowSize, WindowScalePolicy};

struct OpenWindowExample;

impl WindowHandler for OpenWindowExample {
    type Message = ();

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
        size: WindowSize::Logical(baseview::Size::new(512.0, 512.0)),
        scale: WindowScalePolicy::TrySystemScaleFactor,
        parent: baseview::Parent::None,
    };

    let handle = Window::open(window_open_options, |_| OpenWindowExample);
    handle.app_run_blocking();
}
