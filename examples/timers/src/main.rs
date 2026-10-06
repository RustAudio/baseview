use baseview::dpi::LogicalSize;
use baseview::gl::{GlConfig, GlContext};
use baseview::{
    Event, EventStatus, HandlerError, TimerHandle, Window, WindowContext, WindowHandler,
    WindowSettings, WindowSize,
};
use color::{Hsl, OpaqueColor};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};
use std::cell::{Cell, RefCell};
use std::time::Duration;

struct FemtovgExample {
    window_context: WindowContext,
    gl_context: GlContext,
    canvas: RefCell<Canvas<OpenGl>>,

    rect_1_color: Cell<Color>,
    rect_2_color: Cell<Color>,

    rect_1_timer: TimerHandle,
    rect_2_timer: TimerHandle,
}

impl FemtovgExample {
    fn new(window_context: WindowContext) -> Result<Self, HandlerError> {
        let Some(gl_context) = window_context.gl_context() else { unreachable!() };
        unsafe { gl_context.make_current()? };

        let renderer =
            unsafe { OpenGl::new_from_function_cstr(|s| gl_context.get_proc_address(s)) }?;

        let mut canvas = Canvas::new(renderer)?;
        let size = window_context.size();

        canvas.set_size(size.physical.width, size.physical.height, size.scale_factor as f32);

        unsafe { gl_context.make_not_current()? };
        Ok(Self {
            canvas: canvas.into(),

            rect_1_color: new_random_color().into(),
            rect_2_color: new_random_color().into(),

            rect_1_timer: window_context.create_timer(Duration::from_millis(250))?,
            rect_2_timer: window_context.create_timer(Duration::from_millis(1000))?,

            gl_context,
            window_context,
        })
    }
}

impl WindowHandler for FemtovgExample {
    fn draw(&self) -> Result<(), HandlerError> {
        let context = &self.gl_context;
        unsafe { context.make_current()? };

        let mut canvas = self.canvas.borrow_mut();

        let screen_height = canvas.height();
        let screen_width = canvas.width();

        // Clear
        canvas.clear_rect(0, 0, screen_width, screen_height, Color::rgb(0x0A, 0x0A, 0x0A));

        // Make 1st rectangle
        canvas.clear_rect(
            (screen_width as f32 * 0.3).floor() as u32,
            (screen_height as f32 * 0.45).floor() as u32,
            (screen_width as f32 * 0.1).floor() as u32,
            (screen_height as f32 * 0.1).floor() as u32,
            self.rect_1_color.get(),
        );

        // Make 2nd rectangle
        canvas.clear_rect(
            (screen_width as f32 * 0.5).floor() as u32,
            (screen_height as f32 * 0.45).floor() as u32,
            (screen_width as f32 * 0.1).floor() as u32,
            (screen_height as f32 * 0.1).floor() as u32,
            self.rect_2_color.get(),
        );

        // Tell renderer to execute all drawing commands
        canvas.flush();
        context.swap_buffers()?;
        unsafe { context.make_not_current()? };

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        let size = new_size.physical;
        self.canvas.borrow_mut().set_size(size.width, size.height, new_size.scale_factor as f32);

        Ok(())
    }

    fn on_event(&self, _event: Event) -> EventStatus {
        EventStatus::Ignored
    }

    fn on_timer(&self, timer: &TimerHandle) {
        if timer == self.rect_1_timer {
            self.rect_1_color.set(new_random_color());
            self.window_context.request_redraw();
        }

        if timer == self.rect_2_timer {
            self.rect_2_color.set(new_random_color());
            self.window_context.request_redraw();
        }
    }
}

fn main() -> Result<(), baseview::Error> {
    unsafe { baseview::assume_standalone_in_process() };

    let window_open_options = WindowSettings::new()
        .with_title("Baseview Waker example")
        .with_size(LogicalSize::new(512, 512))
        .with_gl_config(GlConfig { alpha_bits: 8, ..GlConfig::default() });

    Window::create(window_open_options, FemtovgExample::new)?.run_until_closed()
}

fn new_random_color() -> Color {
    let hue = rand::random_range(0.0..360.0);
    let hsv_color = OpaqueColor::<Hsl>::new([hue, 100.0, 75.0]);
    let color = hsv_color.to_rgba8();

    Color::rgb(color.r, color.g, color.b)
}
