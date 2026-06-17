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
    rx: Consumer<Message>,

    _ctx: softbuffer::Context,
    surface: softbuffer::Surface,
    current_size: WindowInfo,
    mouse_pos: PhyPoint,
    is_cursor_inside: bool,
    damaged: bool,
}

impl WindowHandler for OpenWindowExample {
    fn on_frame(&mut self, _window: &mut Window) {
        if !self.damaged {
            return;
        }

        let mut pixels = self.surface.buffer_mut().unwrap();
        let size = self.current_size.physical_size();
        let scale_factor = self.current_size.scale();
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

        if self.is_cursor_inside {
            let rect_size = (25.0 * scale_factor) as i32;

            let rect_x_start = (self.mouse_pos.x - rect_size).clamp(0, width as i32) as u32;
            let rect_x_end = (self.mouse_pos.x + rect_size).clamp(0, width as i32) as u32;
            let rect_y_start = (self.mouse_pos.y - rect_size).clamp(0, height as i32) as u32;
            let rect_y_end = (self.mouse_pos.y + rect_size).clamp(0, height as i32) as u32;

            for x in rect_x_start..rect_x_end {
                for y in rect_y_start..rect_y_end {
                    let index = y * width + x;
                    pixels[index as usize] = 0xFF00FF00;
                }
            }
        }

        pixels.present().unwrap();
        self.damaged = false;

        while let Ok(message) = self.rx.pop() {
            println!("Message: {:?}", message);
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            #[cfg(target_os = "macos")]
            Event::Mouse(MouseEvent::ButtonPressed { .. }) => copy_to_clipboard("This is a test!"),
            Event::Mouse(MouseEvent::CursorMoved { position, .. }) => {
                let phy_pos = position.to_physical(&self.current_size);
                self.mouse_pos = phy_pos;
                self.damaged = true;
            }
            Event::Mouse(MouseEvent::CursorEntered) => {
                self.is_cursor_inside = true;
                self.damaged = true;
            }
            Event::Mouse(MouseEvent::CursorLeft) => {
                self.is_cursor_inside = false;
                self.damaged = true;
            }
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Resized: {:?}", info);
                self.current_size = *info;

                let new_size = info.physical_size();

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
            surface,
            rx,
            current_size: WindowInfo::from_physical_size(PhySize::new(512, 512), 1.0),
            mouse_pos: PhyPoint::new(0, 0),
            is_cursor_inside: false,
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
