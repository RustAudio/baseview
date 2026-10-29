use baseview::dpi::{LogicalSize, PhysicalSize, Pixel};
use baseview::gl::{GlConfig, GlContext};
use baseview::{
    AspectRatio, DamageArea, Event, EventStatus, HandlerError, Window, WindowContext,
    WindowHandler, WindowSettings, WindowSize,
};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};
use std::cell::RefCell;

const ASPECT_RATIO: AspectRatio = AspectRatio::new(1, 1);

struct FemtovgExample {
    gl_context: GlContext,
    canvas: RefCell<Canvas<OpenGl>>,
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
        Ok(Self { gl_context, canvas: canvas.into() })
    }
}

impl WindowHandler for FemtovgExample {
    fn draw(&self) -> Result<(), HandlerError> {
        let context = &self.gl_context;
        unsafe { context.make_current()? };

        let mut canvas = self.canvas.borrow_mut();
        let aspect_ratio = ASPECT_RATIO.ratio();

        let canvas_height = canvas.height();
        let canvas_width = canvas.width();

        // If aspect ratio works, these should be equal to the canvas size
        let height_ratioed = (canvas_width as f64 / aspect_ratio).cast();
        let width_ratioed = (canvas_height as f64 * aspect_ratio).cast();

        canvas.clear_rect(0, 0, canvas_width, canvas_height, Color::black());
        canvas.clear_rect(0, height_ratioed, canvas_width, canvas_height, Color::rgb(255, 0, 0));
        canvas.clear_rect(width_ratioed, 0, canvas_width, canvas_height, Color::rgb(255, 0, 0));

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
}

fn main() -> Result<(), baseview::Error> {
    unsafe { baseview::assume_standalone_in_process() };

    let window_open_options = WindowSettings::new()
        .with_title("Baseview Partial redraw example")
        .with_size(LogicalSize::new(512, 512))
        .with_aspect_ratio(Some(ASPECT_RATIO))
        .with_gl_config(GlConfig { alpha_bits: 8, ..GlConfig::default() });

    let window = Window::create(window_open_options, FemtovgExample::new)?;

    window.run_until_closed()?;
    Ok(())
}
