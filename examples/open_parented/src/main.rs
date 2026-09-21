use baseview::dpi::LogicalSize;
use baseview::{
    Event, EventStatus, HandlerError, Window, WindowContext, WindowHandler, WindowSettings,
    WindowSize,
};
use std::cell::RefCell;
use std::num::NonZeroU32;

struct ParentWindowHandler {
    surface: RefCell<softbuffer::Surface<WindowContext, WindowContext>>,
    child_window: Window,
}

impl ParentWindowHandler {
    pub fn new(window: WindowContext) -> Result<Self, HandlerError> {
        let ctx = softbuffer::Context::new(window.clone())?;
        let mut surface = softbuffer::Surface::new(&ctx, window.clone())?;
        let size = window.size().physical;
        surface.resize(size.width.try_into()?, size.height.try_into()?)?;

        let window_open_options =
            WindowSettings::new().with_size(size).with_parent(&window).with_title("baseview child");

        let child_window = Window::create(window_open_options, ChildWindowHandler::new)?;
        child_window.show()?;

        Ok(Self { surface: surface.into(), child_window })
    }
}

impl WindowHandler for ParentWindowHandler {
    fn draw(&self) -> Result<(), HandlerError> {
        let mut surface = self.surface.borrow_mut();
        let mut buf = surface.buffer_mut()?;
        buf.fill(0xFFAA0000);
        buf.present()?;

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        println!("Parent Resized: {new_size:?}");

        if let (Some(width), Some(height)) =
            (NonZeroU32::new(new_size.physical.width), NonZeroU32::new(new_size.physical.height))
        {
            self.surface.borrow_mut().resize(width, height)?;
        }

        self.child_window.suggest_fallback_scale_factor(new_size.scale_factor)?;
        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Mouse(e) => println!("Parent Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Parent Keyboard event: {:?}", e),
            Event::Window(e) => println!("Parent Window event: {:?}", e),
            _ => {}
        }

        EventStatus::Captured
    }
}

struct ChildWindowHandler {
    surface: RefCell<softbuffer::Surface<WindowContext, WindowContext>>,
}

impl ChildWindowHandler {
    pub fn new(window: WindowContext) -> Result<Self, HandlerError> {
        let ctx = softbuffer::Context::new(window.clone())?;
        let mut surface = softbuffer::Surface::new(&ctx, window.clone())?;
        let size = window.size().physical;
        surface.resize(size.width.try_into()?, size.height.try_into()?)?;

        Ok(Self { surface: surface.into() })
    }
}

impl WindowHandler for ChildWindowHandler {
    fn draw(&self) -> Result<(), HandlerError> {
        let mut surface = self.surface.borrow_mut();
        let mut buf = surface.buffer_mut()?;
        buf.fill(0xFFAAAAAA);
        buf.present()?;

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        println!("Child Resized: {new_size:?}");

        if let (Some(width), Some(height)) =
            (NonZeroU32::new(new_size.physical.width), NonZeroU32::new(new_size.physical.height))
        {
            self.surface.borrow_mut().resize(width, height)?;
        }

        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Mouse(e) => println!("Child Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Child Keyboard event: {:?}", e),
            Event::Window(e) => println!("Child Window event: {:?}", e),
            _ => {}
        }

        EventStatus::Captured
    }
}

fn main() -> Result<(), baseview::Error> {
    unsafe { baseview::assume_standalone_in_process() };
    let window_open_options = WindowSettings::new().with_size(LogicalSize::new(512.0, 512.0));

    Window::create(window_open_options, ParentWindowHandler::new)?.run_until_closed()?;

    Ok(())
}
