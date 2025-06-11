use std::sync::Arc;
use std::{num::NonZeroU32};
use std::time::Duration;

use rtrb::{Consumer, RingBuffer};

#[cfg(target_os = "macos")]
use baseview::{copy_to_clipboard, MouseEvent};
use baseview::{
    Event, EventStatus, PhySize, Window, WindowEvent, WindowHandler, WindowScalePolicy,
};

#[derive(Debug, Clone)]
enum Message {
    Hello,
}

struct OpenWindowExample {
    rx: Consumer<Message>,

    _ctx: softbuffer::Context<Arc<Window<'static>>>,
    surface: softbuffer::Surface<
        Arc<Window<'static>>,
        Arc<Window<'static>>
    >,
    current_size: PhySize,
    damaged: bool,
}


impl WindowHandler for OpenWindowExample {
    fn on_frame(&mut self, _window: &mut Window) {
        let mut buf = self.surface.buffer_mut().unwrap();
        if self.damaged {
            buf.fill(0xFFAAAAAA);
            self.damaged = false;
        }
        buf.present().unwrap();

        while let Ok(message) = self.rx.pop() {
            println!("Message: {:?}", message);
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            #[cfg(target_os = "macos")]
            Event::Mouse(MouseEvent::ButtonPressed { .. }) => copy_to_clipboard("This is a test!"),
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size = new_size;

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height))
                {
                    self.surface.resize(width, height).unwrap();
                    self.damaged = true;
                }
            }
            _ => {}
        }

        log_event(&event);

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

    let (mut tx, rx) = RingBuffer::new(128);

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));

        if tx.push(Message::Hello).is_err() {
            println!("Failed sending message");
        }
    });

    Window::open_blocking(window_open_options, |window| {
        // Create Arc from the window with 'static lifetime
        let window_arc: Arc<Window<'static>> = Arc::new(unsafe {
            std::mem::transmute(window)
        });
        
        let ctx = softbuffer::Context::new(window_arc.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&ctx, window_arc.clone()).unwrap();
        surface.resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap()).unwrap();

        OpenWindowExample {
            _ctx: ctx,
            surface,
            rx,
            current_size: PhySize::new(512, 512),
            damaged: true,
        }
    });
}

fn log_event(event: &Event) {
    match event {
        Event::Mouse(e) => println!("Mouse event: {:?}", e),
        Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
        Event::Window(e) => println!("Window event: {:?}", e),
    }
}
