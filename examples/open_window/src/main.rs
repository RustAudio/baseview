use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use std::time::Duration;

use rtrb::{Consumer, RingBuffer};

#[cfg(target_os = "macos")]
use baseview::copy_to_clipboard;
use baseview::{
    Event, EventStatus, MouseEvent, PhyPoint, PhySize, Window, WindowEvent, WindowHandler,
    WindowInfo, WindowOpenOptions,
};

#[derive(Debug, Clone)]
enum Message {
    Hello,
}

struct OpenWindowExample {
    rx: RefCell<Consumer<Message>>,

    _ctx: softbuffer::Context,
    surface: RefCell<softbuffer::Surface>,
    current_size: Cell<WindowInfo>,
    mouse_pos: Cell<PhyPoint>,
    is_cursor_inside: Cell<bool>,
    damaged: Cell<bool>,
}

impl WindowHandler for OpenWindowExample {
    fn on_frame(&self, _window: &mut Window) {
        if !self.damaged.get() {
            return;
        }

        let mut surface = self.surface.borrow_mut();
        let mut pixels = surface.buffer_mut().unwrap();
        let size = self.current_size.get().physical_size();
        let scale_factor = self.current_size.get().scale();
        let (width, height) = (size.width, size.height);

        for index in 0..(width * height) {
            let x = index % width;
            let y = index / width;

            let red = ((x as f32 / width as f32) * 255.0) as u32;
            let green = ((y as f32 / height as f32) * 255.0) as u32;
            let blue = (((x * y) as f64 / scale_factor) as u32) % 255;

            pixels[index as usize] = blue | (green << 8) | (red << 16) | 0xFF000000;
        }

        for x in 0..width {
            // Green line on top
            let y = 0;
            let index = y * width + x;
            pixels[index as usize] = if (x % 10) < 5 { 0xFF00FF00 } else { 0xFF000000 };

            // Magenta line on bottom
            let y = height - 1;
            let index = y * width + x;
            pixels[index as usize] = if (x % 10) < 5 { 0xFFFF00FF } else { 0xFF000000 };
        }

        for y in 0..height {
            // Green line on right
            let x = width - 1;
            let index = y * width + x;
            pixels[index as usize] = if (y % 10) < 5 { 0xFF00FF00 } else { 0xFF000000 };

            // Magenta line on left
            let x = 0;
            let index = y * width + x;
            pixels[index as usize] = if (y % 10) < 5 { 0xFFFF00FF } else { 0xFF000000 };
        }

        if self.is_cursor_inside.get() {
            let rect_size = (25.0 * scale_factor) as i32;

            let rect_x_start = (self.mouse_pos.get().x - rect_size).clamp(0, width as i32) as u32;
            let rect_x_end = (self.mouse_pos.get().x + rect_size).clamp(0, width as i32) as u32;
            let rect_y_start = (self.mouse_pos.get().y - rect_size).clamp(0, height as i32) as u32;
            let rect_y_end = (self.mouse_pos.get().y + rect_size).clamp(0, height as i32) as u32;

            for x in rect_x_start..rect_x_end {
                for y in rect_y_start..rect_y_end {
                    let index = y * width + x;
                    pixels[index as usize] = 0xFF00FF00;
                }
            }
        }

        pixels.present().unwrap();
        self.damaged.set(false);

        while let Ok(message) = self.rx.borrow_mut().pop() {
            println!("Message: {:?}", message);
        }
    }

    fn on_event(&self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            #[cfg(target_os = "macos")]
            Event::Mouse(MouseEvent::ButtonPressed { .. }) => copy_to_clipboard("This is a test!"),
            Event::Mouse(MouseEvent::CursorMoved { position, .. }) => {
                let phy_pos = position.to_physical(&self.current_size.get());
                self.mouse_pos.set(phy_pos);
                self.damaged.set(true);
            }
            Event::Mouse(MouseEvent::CursorEntered) => {
                self.is_cursor_inside.set(true);
                self.damaged.set(true);
            }
            Event::Mouse(MouseEvent::CursorLeft) => {
                self.is_cursor_inside.set(false);
                self.damaged.set(true);
            }
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Resized: {:?}", info);
                self.current_size.set(*info);

                let new_size = info.physical_size();

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height))
                {
                    self.surface.borrow_mut().resize(width, height).unwrap();
                    self.damaged.set(true);
                }
            }
            _ => {}
        }

        log_event(&event);

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowOpenOptions::new().with_size(512.0, 512.0);

    let (mut tx, rx) = RingBuffer::new(128);

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));

        if tx.push(Message::Hello).is_err() {
            println!("Failed sending message");
        }
    });

    Window::open_blocking(window_open_options, |window| {
        let ctx = unsafe { softbuffer::Context::new(window) }.unwrap();
        let mut surface = unsafe { softbuffer::Surface::new(&ctx, window) }.unwrap();
        surface.resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap()).unwrap();

        OpenWindowExample {
            _ctx: ctx,
            surface: surface.into(),
            rx: rx.into(),
            current_size: WindowInfo::from_physical_size(PhySize::new(512, 512).into(), 1.0).into(),
            mouse_pos: PhyPoint::new(0, 0).into(),
            is_cursor_inside: false.into(),
            damaged: true.into(),
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
