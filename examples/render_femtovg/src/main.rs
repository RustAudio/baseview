use baseview::gl::GlConfig;
use baseview::{
    Event, EventStatus, MouseEvent, PhyPoint, Size, Window, WindowEvent, WindowHandler, WindowInfo,
    WindowOpenOptions, WindowScalePolicy,
};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};

struct FemtovgExample {
    canvas: Canvas<OpenGl>,
    current_size: WindowInfo,
    current_mouse_position: PhyPoint,
    damaged: bool,
}

impl FemtovgExample {
    fn new(window: &mut Window) -> Self {
        let context = window.gl_context().unwrap();
        unsafe { context.make_current() };

        let renderer =
            unsafe { OpenGl::new_from_function(|s| context.get_proc_address(s)) }.unwrap();

        let mut canvas = Canvas::new(renderer).unwrap();
        // TODO: get actual window width
        canvas.set_size(512, 512, 1.0);

        unsafe { context.make_not_current() };
        Self {
            canvas,
            current_size: WindowInfo::from_logical_size(Size { width: 512.0, height: 512.0 }, 1.0),
            current_mouse_position: PhyPoint { x: 256, y: 256 },
            damaged: true,
        }
    }
}

impl WindowHandler for FemtovgExample {
    fn on_frame(&mut self, window: &mut Window) {
        if !self.damaged {
            return;
        }

        let context = window.gl_context().unwrap();
        unsafe { context.make_current() };

        let screen_height = self.canvas.height();
        let screen_width = self.canvas.width();

        // Clear
        self.canvas.clear_rect(0, 0, screen_width, screen_height, Color::rgb(0xAA, 0xAA, 0xAA));

        // Make big blue rectangle
        self.canvas.clear_rect(
            (screen_width as f32 * 0.1).floor() as u32,
            (screen_height as f32 * 0.1).floor() as u32,
            (screen_width as f32 * 0.8).floor() as u32,
            (screen_height as f32 * 0.8).floor() as u32,
            Color::rgbf(0., 0.3, 0.9),
        );

        // Make smol orange rectangle
        self.canvas.clear_rect(
            (self.current_mouse_position.x - 15).clamp(0, screen_width as i32 - 30) as u32,
            (self.current_mouse_position.y - 15).clamp(0, screen_height as i32 - 30) as u32,
            30,
            30,
            Color::rgbf(0.9, 0.3, 0.),
        );

        // Tell renderer to execute all drawing commands
        self.canvas.flush();
        context.swap_buffers();
        unsafe { context.make_not_current() };
        self.damaged = false;
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(size)) => {
                let phy_size = size.physical_size();
                self.current_size = size;
                self.canvas.set_size(phy_size.width, phy_size.height, size.scale() as f32);
                self.damaged = true;
            }
            Event::Mouse(MouseEvent::CursorMoved { position, .. }) => {
                self.current_mouse_position = position.to_physical(&self.current_size);
                self.damaged = true;
            }
            _ => {}
        };
        log_event(&event);
        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowOpenOptions {
        title: "Femtovg on Baseview".into(),
        size: Size::new(512.0, 512.0),
        scale: WindowScalePolicy::SystemScaleFactor,

        gl_config: Some(GlConfig { alpha_bits: 8, ..GlConfig::default() }),
    };

    Window::open_blocking(window_open_options, FemtovgExample::new);
}

fn log_event(event: &Event) {
    match event {
        Event::Mouse(e) => println!("Mouse event: {:?}", e),
        Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
        Event::Window(e) => println!("Window event: {:?}", e),
    }
}
