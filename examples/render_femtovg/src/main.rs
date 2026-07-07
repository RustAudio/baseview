use baseview::dpi::{LogicalSize, PhysicalPosition};
use baseview::gl::{GlConfig, GlContext};
use baseview::{
    Event, EventStatus, MouseEvent, WindowBuilder, WindowContext, WindowHandler, WindowSize,
};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};
use std::cell::{Cell, RefCell};

struct FemtovgExample {
    window_context: WindowContext,
    gl_context: GlContext,
    canvas: RefCell<Canvas<OpenGl>>,
    current_mouse_position: Cell<PhysicalPosition<f64>>,
    damaged: Cell<bool>,
}

impl FemtovgExample {
    fn new(window_context: WindowContext) -> Self {
        let gl_context = window_context.gl_context().unwrap();
        unsafe { gl_context.make_current() };

        let renderer =
            unsafe { OpenGl::new_from_function(|s| gl_context.get_proc_address(s)) }.unwrap();

        let mut canvas = Canvas::new(renderer).unwrap();
        let size = window_context.size();

        canvas.set_size(size.physical.width, size.physical.height, size.scale_factor as f32);

        unsafe { gl_context.make_not_current() };
        Self {
            gl_context,
            window_context,
            canvas: canvas.into(),
            damaged: true.into(),
            current_mouse_position: Cell::new(PhysicalPosition::default()),
        }
    }
}

impl WindowHandler for FemtovgExample {
    fn on_frame(&self) {
        if !self.damaged.get() {
            return;
        }

        let context = &self.gl_context;
        unsafe { context.make_current() };

        let mut canvas = self.canvas.borrow_mut();

        let screen_height = canvas.height();
        let screen_width = canvas.width();

        // Clear
        canvas.clear_rect(0, 0, screen_width, screen_height, Color::rgb(0xAA, 0xAA, 0xAA));

        // Make big blue rectangle
        canvas.clear_rect(
            (screen_width as f32 * 0.1).floor() as u32,
            (screen_height as f32 * 0.1).floor() as u32,
            (screen_width as f32 * 0.8).floor() as u32,
            (screen_height as f32 * 0.8).floor() as u32,
            Color::rgbf(0., 0.3, 0.9),
        );

        let mouse_position = self.current_mouse_position.get().cast::<i32>();

        // Make smol orange rectangle
        canvas.clear_rect(
            (mouse_position.x - 15).clamp(0, screen_width as i32 - 30) as u32,
            (mouse_position.y - 15).clamp(0, screen_height as i32 - 30) as u32,
            30,
            30,
            Color::rgbf(0.9, 0.3, 0.),
        );

        // Tell renderer to execute all drawing commands
        canvas.flush();
        context.swap_buffers();
        unsafe { context.make_not_current() };
        self.damaged.set(false);
    }

    fn resized(&self, new_size: WindowSize) {
        let size = new_size.physical;
        self.canvas.borrow_mut().set_size(size.width, size.height, new_size.scale_factor as f32);
        self.damaged.set(true);
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Mouse(
                MouseEvent::CursorMoved { position, .. }
                | MouseEvent::DragEntered { position, .. }
                | MouseEvent::DragMoved { position, .. }
                | MouseEvent::DragDropped { position, .. },
            ) => {
                self.current_mouse_position.set(position);
                if position.y > 400. && !self.window_context.has_focus() {
                    self.window_context.focus()
                }
                self.damaged.set(true);
            }
            event => log_event(&event),
        };

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowBuilder::new()
        .with_title("Femtovg on Baseview")
        .with_size(LogicalSize::new(512, 512))
        .with_gl_config(GlConfig { alpha_bits: 8, ..GlConfig::default() });

    baseview::create_window(window_open_options, FemtovgExample::new).run_until_closed().unwrap();
}

fn log_event(event: &Event) {
    match event {
        Event::Mouse(e) => println!("Mouse event: {:?}", e),
        Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
        Event::Window(e) => println!("Window event: {:?}", e),
        _ => {}
    }
}
