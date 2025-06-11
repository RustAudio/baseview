use baseview::{
    Event, EventStatus, PhySize, Window, WindowEvent, WindowHandle, WindowHandler,
    WindowScalePolicy,
};
use std::num::NonZeroU32;
use std::sync::Arc;

struct ParentWindowHandler {
    _ctx: softbuffer::Context<Arc<Window<'static>>>,
    surface: softbuffer::Surface<Arc<Window<'static>>, Arc<Window<'static>>>,
    current_size: PhySize,
    damaged: bool,

    _child_window: Option<WindowHandle>,
}

impl ParentWindowHandler {
    pub fn new(window: &mut Window) -> Self {
        let window_open_options = baseview::WindowOpenOptions {
            title: "baseview child".into(),
            size: baseview::Size::new(256.0, 256.0),
            scale: WindowScalePolicy::SystemScaleFactor,

            // TODO: Add an example that uses the OpenGL context
            #[cfg(feature = "opengl")]
            gl_config: None,
        };
        let child_window =
            Window::open_parented(window, window_open_options, ChildWindowHandler::new);

        let window_arc: Arc<Window<'static>> = Arc::new(unsafe {
            std::mem::transmute(window)
        });
        
        let ctx = softbuffer::Context::new(window_arc.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&ctx, window_arc.clone()).unwrap();
        surface.resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap()).unwrap();

        // TODO: no way to query physical size initially?
        Self {
            _ctx: ctx,
            surface,
            current_size: PhySize::new(512, 512),
            damaged: true,
            _child_window: Some(child_window),
        }
    }
}

impl WindowHandler for ParentWindowHandler {
    fn on_frame(&mut self, _window: &mut Window) {
        let mut buf = self.surface.buffer_mut().unwrap();
        if self.damaged {
            buf.fill(0xFFAAAAAA);
            self.damaged = false;
        }
        buf.present().unwrap();
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Parent Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size = new_size;

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height))
                {
                    self.surface.resize(width, height).unwrap();
                    self.damaged = true;
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
    _ctx: softbuffer::Context<Arc<Window<'static>>>,
    surface: softbuffer::Surface<Arc<Window<'static>>, Arc<Window<'static>>>,
    current_size: PhySize,
    damaged: bool,
}

impl ChildWindowHandler {
    pub fn new(window: &mut Window) -> Self {
        let window_arc: Arc<Window<'static>> = Arc::new(unsafe {
            std::mem::transmute(window)
        });
        
        let ctx = softbuffer::Context::new(window_arc.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&ctx, window_arc.clone()).unwrap();
        surface.resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap()).unwrap();

        // TODO: no way to query physical size initially?
        Self { _ctx: ctx, surface, current_size: PhySize::new(256, 256), damaged: true }
    }
}

impl WindowHandler for ChildWindowHandler {
    fn on_frame(&mut self, _window: &mut Window) {
        let mut buf = self.surface.buffer_mut().unwrap();
        if self.damaged {
            buf.fill(0xFFAA0000);
            self.damaged = false;
        }
        buf.present().unwrap();
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Child Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size = new_size;

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height))
                {
                    self.surface.resize(width, height).unwrap();
                    self.damaged = true;
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
    let window_open_options = baseview::WindowOpenOptions {
        title: "baseview".into(),
        size: baseview::Size::new(512.0, 512.0),
        scale: WindowScalePolicy::SystemScaleFactor,

        // TODO: Add an example that uses the OpenGL context
        #[cfg(feature = "opengl")]
        gl_config: None,
    };

    Window::open_blocking(window_open_options, ParentWindowHandler::new);
}
