use baseview::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use baseview::gl::{GlConfig, GlContext};
use baseview::{
    DamageArea, Event, EventStatus, HandlerError, Window, WindowContext, WindowHandler,
    WindowSettings, WindowSize,
};
use color::{Hsl, OpaqueColor};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};
use std::cell::{Cell, RefCell};

struct FemtovgExample {
    gl_context: GlContext,
    canvas: RefCell<Canvas<OpenGl>>,
    damage_handler: DamageHandler,
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
        Ok(Self { gl_context, canvas: canvas.into(), damage_handler: DamageHandler::new() })
    }
}

impl WindowHandler for FemtovgExample {
    fn draw(&self) -> Result<(), HandlerError> {
        let context = &self.gl_context;
        unsafe { context.make_current()? };

        let mut canvas = self.canvas.borrow_mut();

        let screen_height = canvas.height();
        let screen_width = canvas.width();

        let color_of_the_day = new_random_color();

        if self.damage_handler.is_full_screen_clear() {
            canvas.clear_rect(0, 0, screen_width, screen_height, color_of_the_day);
        } else {
            while let Some((pos, size)) = self.damage_handler.next_damage() {
                dbg!((pos, size));
                canvas.clear_rect(pos.x, pos.y, size.width, size.height, color_of_the_day);
            }
        }

        self.damage_handler.clear();

        // Tell renderer to execute all drawing commands
        canvas.flush();
        context.swap_buffers()?;
        unsafe { context.make_not_current()? };

        Ok(())
    }

    fn damage(&self, area: DamageArea) {
        eprintln!("Damaged: {area:?}");
        match area {
            DamageArea::FullWindow => self.damage_handler.set_full_screen_clear(),
            DamageArea::Rect { position, size } => {
                self.damage_handler.add_damaged_rect(position, size)
            }
            _ => {}
        }
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
        .with_gl_config(GlConfig { alpha_bits: 8, ..GlConfig::default() });

    let window = Window::create(window_open_options, |ctx| FemtovgExample::new(ctx))?;

    window.run_until_closed()?;
    Ok(())
}

fn new_random_color() -> Color {
    let hue = rand::random_range(0.0..360.0);
    let hsv_color = OpaqueColor::<Hsl>::new([hue, 100.0, 15.0]);
    let color = hsv_color.to_rgba8();

    Color::rgb(color.r, color.g, color.b)
}

struct DamageHandler {
    full_screen: Cell<bool>,
    damaged_rects: RefCell<Vec<(PhysicalPosition<u32>, PhysicalSize<u32>)>>,
}

impl DamageHandler {
    fn new() -> Self {
        Self { full_screen: false.into(), damaged_rects: RefCell::new(Vec::with_capacity(8)) }
    }

    fn is_full_screen_clear(&self) -> bool {
        self.full_screen.get()
    }

    fn set_full_screen_clear(&self) {
        self.full_screen.set(true)
    }

    fn clear(&self) {
        self.full_screen.set(false);
        self.damaged_rects.borrow_mut().clear();
    }

    fn next_damage(&self) -> Option<(PhysicalPosition<u32>, PhysicalSize<u32>)> {
        self.damaged_rects.borrow_mut().pop()
    }

    fn add_damaged_rect(&self, position: PhysicalPosition<u32>, size: PhysicalSize<u32>) {
        self.damaged_rects.borrow_mut().push((position, size));
    }
}
