use baseview::{
    Event, EventStatus, PhySize, Window, WindowEvent, WindowHandle, WindowHandler,
    WindowOpenOptions,
};
use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;

struct ParentWindowHandler {
    _ctx: softbuffer::Context,
    surface: RefCell<softbuffer::Surface>,
    current_size: Cell<PhySize>,
    damaged: Cell<bool>,

    _child_window: Option<WindowHandle>,
}

impl ParentWindowHandler {
    pub fn new(window: &mut Window) -> Self {
        let ctx = unsafe { softbuffer::Context::new(window) }.unwrap();
        let mut surface = unsafe { softbuffer::Surface::new(&ctx, window) }.unwrap();
        surface.resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap()).unwrap();

        let window_open_options =
            WindowOpenOptions::new().with_size(256.0, 256.0).with_title("baseview child");

        let child_window =
            Window::open_parented(window, window_open_options, ChildWindowHandler::new);

        // TODO: no way to query physical size initially?
        Self {
            _ctx: ctx,
            surface: surface.into(),
            current_size: PhySize::new(512, 512).into(),
            damaged: true.into(),
            _child_window: Some(child_window),
        }
    }
}

impl WindowHandler for ParentWindowHandler {
    fn on_frame(&self, _window: &mut Window) {
        let mut surface = self.surface.borrow_mut();
        let mut buf = surface.buffer_mut().unwrap();
        if self.damaged.get() {
            buf.fill(0xFFAAAAAA);
            self.damaged.set(false);
        }
        buf.present().unwrap();
    }

    fn on_event(&self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Parent Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size.set(new_size);

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height))
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
    _ctx: softbuffer::Context,
    surface: RefCell<softbuffer::Surface>,
    current_size: Cell<PhySize>,
    damaged: Cell<bool>,
}

impl ChildWindowHandler {
    pub fn new(window: &mut Window) -> Self {
        let ctx = unsafe { softbuffer::Context::new(window) }.unwrap();
        let mut surface = unsafe { softbuffer::Surface::new(&ctx, window) }.unwrap();
        surface.resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap()).unwrap();

        // TODO: no way to query physical size initially?
        Self {
            _ctx: ctx,
            surface: surface.into(),
            current_size: PhySize::new(256, 256).into(),
            damaged: true.into(),
        }
    }
}

impl WindowHandler for ChildWindowHandler {
    fn on_frame(&self, _window: &mut Window) {
        let mut surface = self.surface.borrow_mut();
        let mut buf = surface.buffer_mut().unwrap();
        if self.damaged.get() {
            buf.fill(0xFFAA0000);
            self.damaged.set(false);
        }
        buf.present().unwrap();
    }

    fn on_event(&self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Child Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size.set(new_size);

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height))
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
    let window_open_options = WindowOpenOptions::new().with_size(512.0, 512.0);

    Window::open_blocking(window_open_options, ParentWindowHandler::new);
}
