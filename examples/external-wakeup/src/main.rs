use baseview::dpi::LogicalSize;
use baseview::gl::{GlConfig, GlContext};
use baseview::{
    Event, EventStatus, HandlerError, Window, WindowContext, WindowHandler, WindowSettings,
    WindowSize, WindowWaker,
};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};
use std::cell::{Cell, RefCell};
use std::sync::mpsc::*;
use std::time::Duration;

#[derive(Copy, Clone, Debug)]
enum Message {
    Hello,
}

struct FemtovgExample {
    window_context: WindowContext,
    gl_context: GlContext,
    canvas: RefCell<Canvas<OpenGl>>,

    green_rect_opacity: Cell<f32>,

    receiver: Receiver<Message>,
}

impl FemtovgExample {
    fn new(
        window_context: WindowContext, receiver: Receiver<Message>,
    ) -> Result<Self, HandlerError> {
        let Some(gl_context) = window_context.gl_context() else { unreachable!() };
        unsafe { gl_context.make_current()? };

        let renderer =
            unsafe { OpenGl::new_from_function_cstr(|s| gl_context.get_proc_address(s)) }?;

        let mut canvas = Canvas::new(renderer)?;
        let size = window_context.size();

        canvas.set_size(size.physical.width, size.physical.height, size.scale_factor as f32);

        unsafe { gl_context.make_not_current()? };
        Ok(Self {
            gl_context,
            window_context,
            canvas: canvas.into(),
            green_rect_opacity: 0.0.into(),
            receiver,
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

        if self.green_rect_opacity.get() <= 0.0 {
            // Make orange rectangle
            canvas.clear_rect(
                (screen_width as f32 * 0.3).floor() as u32,
                (screen_height as f32 * 0.45).floor() as u32,
                (screen_width as f32 * 0.1).floor() as u32,
                (screen_height as f32 * 0.1).floor() as u32,
                Color::rgbf(1.0, 0.5, 0.),
            );
        } else {
            // Make green rectangle
            canvas.clear_rect(
                (screen_width as f32 * 0.5).floor() as u32,
                (screen_height as f32 * 0.45).floor() as u32,
                (screen_width as f32 * 0.1).floor() as u32,
                (screen_height as f32 * 0.1).floor() as u32,
                Color::rgbf(0.0, 1. * self.green_rect_opacity.get(), 0.),
            );

            // Prepare a next frame to animate it fading out
            self.green_rect_opacity.set(self.green_rect_opacity.get() - 1.0 / 60.0);
            self.window_context.request_redraw();
        }

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

    fn poll(&self) {
        let msg = match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => return eprintln!("Channel disconnected!"),
            Ok(msg) => msg,
        };

        eprintln!("Message received: {msg:?}!");
        self.green_rect_opacity.set(1.0);
        self.window_context.request_redraw();
    }
}

fn main() -> Result<(), baseview::Error> {
    unsafe { baseview::assume_standalone_in_process() };
    let (sender, receiver) = channel();

    let window_open_options = WindowSettings::new()
        .with_title("Baseview Waker example")
        .with_size(LogicalSize::new(512, 512))
        .with_gl_config(GlConfig { alpha_bits: 8, ..GlConfig::default() });

    let window = Window::create(window_open_options, |ctx| FemtovgExample::new(ctx, receiver))?;
    let waker = window.waker();
    std::thread::spawn(|| run_thread(sender, waker));

    window.run_until_closed()?;
    Ok(())
}

fn run_thread(sender: Sender<Message>, waker: WindowWaker) {
    loop {
        let interval: f32 = rand::random_range(0.5..2.5);
        std::thread::sleep(Duration::from_secs_f32(interval));
        sender.send(Message::Hello).unwrap();
        waker.request_poll();
    }
}
