use crate::perf::PerfGraph;
use baseview::dpi::LogicalSize;
use baseview::gl::{GlConfig, GlContext};
use baseview::{
    Event, EventStatus, HandlerError, Window, WindowContext, WindowHandler, WindowSettings,
    WindowSize,
};
use femtovg::renderer::OpenGl;
use femtovg::{Align, Baseline, Canvas, Color, Paint, Renderer};
use keyboard_types::{Key, KeyState, KeyboardEvent, NamedKey};
use std::cell::{Cell, RefCell};
use std::time::Instant;

mod perf;

const BAR_WIDTH: u32 = 10;
const BAR_COUNT: u32 = 5;
const BAR_SPEED_INCREMENTS: u32 = 3;

struct FramePacingTest {
    window_context: WindowContext,
    gl_context: GlContext,
    canvas: RefCell<Canvas<OpenGl>>,
    perf_graph: PerfGraph,
    previous_frame_time: Cell<Instant>,

    bar_pos: Cell<u32>,
    bar_speed: Cell<u32>,
}

impl FramePacingTest {
    fn new(window_context: WindowContext) -> Result<Self, HandlerError> {
        let Some(gl_context) = window_context.gl_context() else { unreachable!() };
        unsafe { gl_context.make_current()? };

        let renderer =
            unsafe { OpenGl::new_from_function_cstr(|s| gl_context.get_proc_address(s)) }?;

        let mut canvas = Canvas::new(renderer)?;
        let size = window_context.size();

        canvas.set_size(size.physical.width, size.physical.height, size.scale_factor as f32);

        canvas
            .add_font_mem(include_bytes!("../RobotoFlex-VariableFont.ttf"))
            .expect("Cannot add font");

        unsafe { gl_context.make_not_current()? };
        Ok(Self {
            gl_context,
            window_context,
            canvas: canvas.into(),
            perf_graph: PerfGraph::new(),
            previous_frame_time: Instant::now().into(),
            bar_pos: 0.into(),
            bar_speed: 6.into(),
        })
    }
}

impl WindowHandler for FramePacingTest {
    fn on_frame(&self) -> Result<(), HandlerError> {
        let now = Instant::now();
        let dt = (now - self.previous_frame_time.get()).as_secs_f32();
        self.previous_frame_time.set(now);

        self.perf_graph.update(dt);

        unsafe { self.gl_context.make_current()? };

        let mut canvas = self.canvas.borrow_mut();

        let screen_height = canvas.height();
        let screen_width = canvas.width();

        // Clear
        canvas.clear_rect(0, 0, screen_width, screen_height, Color::black());

        // Draw bar

        let spacing = (screen_width - (BAR_WIDTH * BAR_COUNT)) / (BAR_COUNT) + BAR_WIDTH;

        for i in 0..BAR_COUNT {
            draw_bar_may_split(
                &mut canvas,
                self.bar_pos.get() + i * spacing,
                screen_width,
                screen_height,
                BAR_WIDTH,
            );
        }

        // Move bar
        let bar_speed = self.bar_speed.get();

        self.bar_pos.set((self.bar_pos.get() + bar_speed) % screen_width);

        // Extras

        let text_paint = Paint::color(Color::rgba(240, 240, 240, 255))
            .with_font_size(14.0)
            .with_text_align(Align::Left)
            .with_text_baseline(Baseline::Bottom);

        let _ = canvas.fill_text(
            5.0,
            screen_height as f32 - 20.0,
            format!("Speed: {} pixels/sec", bar_speed),
            &text_paint,
        );

        canvas.restore();

        canvas.save();
        canvas.reset();
        self.perf_graph.render(&mut canvas, 5.0, 5.0);
        canvas.restore();

        // Tell renderer to execute all drawing commands
        canvas.flush();
        self.gl_context.swap_buffers()?;
        unsafe { self.gl_context.make_not_current()? };

        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        dbg!(new_size);
        let size = new_size.physical;
        self.canvas.borrow_mut().set_size(size.width, size.height, new_size.scale_factor as f32);

        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        //dbg!(&event);
        if let Event::Keyboard(KeyboardEvent { key, state: KeyState::Down, .. }) = event {
            match key {
                Key::Named(NamedKey::ArrowLeft) => {
                    self.bar_speed.set(self.bar_speed.get().saturating_sub(BAR_SPEED_INCREMENTS))
                }
                Key::Named(NamedKey::ArrowRight) => {
                    self.bar_speed.set(self.bar_speed.get().saturating_add(BAR_SPEED_INCREMENTS))
                }
                _ => {}
            }
        }

        EventStatus::Captured
    }
}

fn main() -> Result<(), baseview::Error> {
    tracing_subscriber::fmt::init();

    let window_open_options = WindowSettings::new()
        .with_title("Femtovg on Baseview")
        .with_size(LogicalSize::new(512, 512))
        .with_gl_config(GlConfig { alpha_bits: 8, vsync: true, ..GlConfig::default() });

    Window::create(window_open_options, FramePacingTest::new)?.run_until_closed()?;
    Ok(())
}

fn draw_bar_may_split(
    canvas: &mut Canvas<impl Renderer>, mut pos: u32, screen_width: u32, screen_height: u32,
    bar_width: u32,
) {
    pos %= screen_width;
    canvas.clear_rect(pos, 0, BAR_WIDTH, screen_height, Color::white());

    if pos + BAR_WIDTH > screen_width {
        canvas.clear_rect(0, 0, BAR_WIDTH - (screen_width - pos), screen_height, Color::white());
    }
}
