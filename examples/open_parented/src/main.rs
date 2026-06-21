use baseview::dpi::LogicalSize;
use baseview::{
    Event, EventStatus, Window, WindowContext, WindowEvent, WindowHandle, WindowHandler,
    WindowOpenOptions,
};
use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;

struct ParentWindowHandler {
    surface: RefCell<softbuffer::Surface<WindowContext, WindowContext>>,
    damaged: Cell<bool>,

    _child_window: Option<WindowHandle>,
}

impl ParentWindowHandler {
    pub fn new(window: WindowContext) -> Self {
        let ctx = softbuffer::Context::new(window.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&ctx, window.clone()).unwrap();
        let size = window.size().physical;
        surface
            .resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap())
            .unwrap();

        let window_open_options = WindowOpenOptions::new()
            .with_size(LogicalSize::new(256, 256))
            .with_title("baseview child");

        let child_window =
            Window::open_parented(&window, window_open_options, ChildWindowHandler::new);

        Self { surface: surface.into(), damaged: true.into(), _child_window: Some(child_window) }
    }
}

impl WindowHandler for ParentWindowHandler {
    fn on_frame(&self) {
        let mut surface = self.surface.borrow_mut();
        let mut buf = surface.buffer_mut().unwrap();
        if self.damaged.get() {
            buf.fill(0xFFAAAAAA);
            self.damaged.set(false);
        }
        buf.present().unwrap();
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized { size, scale_factor }) => {
                println!("Parent Resized: {size:?}, scale: {scale_factor}");

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.surface.borrow_mut().resize(width, height).unwrap();
                    self.damaged.set(true);
                }
            }
            Event::Mouse(e) => println!("Parent Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Parent Keyboard event: {:?}", e),
            Event::Window(e) => println!("Parent Window event: {:?}", e),
        }

        EventStatus::Captured
    }
}

struct ChildWindowHandler {
    surface: RefCell<softbuffer::Surface<WindowContext, WindowContext>>,
    damaged: Cell<bool>,
}

impl ChildWindowHandler {
    pub fn new(window: WindowContext) -> Self {
        let ctx = softbuffer::Context::new(window.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&ctx, window.clone()).unwrap();
        let size = window.size().physical;
        surface
            .resize(NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap())
            .unwrap();

        Self { surface: surface.into(), damaged: true.into() }
    }
}

impl WindowHandler for ChildWindowHandler {
    fn on_frame(&self) {
        let mut surface = self.surface.borrow_mut();
        let mut buf = surface.buffer_mut().unwrap();
        if self.damaged.get() {
            buf.fill(0xFFAA0000);
            self.damaged.set(false);
        }
        buf.present().unwrap();
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized { size, scale_factor }) => {
                println!("Child Resized: {size:?}, scale: {scale_factor}");

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.surface.borrow_mut().resize(width, height).unwrap();
                    self.damaged.set(true);
                }
            }
            Event::Mouse(e) => println!("Child Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Child Keyboard event: {:?}", e),
            Event::Window(e) => println!("Child Window event: {:?}", e),
        }

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowOpenOptions::new().with_size(LogicalSize::new(512.0, 512.0));

    Window::open_blocking(window_open_options, ParentWindowHandler::new);
}
