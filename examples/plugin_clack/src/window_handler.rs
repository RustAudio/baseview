use baseview::dpi::PhysicalPosition;
use baseview::{
    Event, EventStatus, HandlerError, MouseEvent, WindowContext, WindowHandler, WindowSize,
};
use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;

pub struct OpenWindowExample {
    window_context: WindowContext,

    surface: RefCell<softbuffer::Surface<WindowContext, WindowContext>>,
    mouse_pos: Cell<PhysicalPosition<f64>>,
    is_cursor_inside: Cell<bool>,
    damaged: Cell<bool>,
}

impl WindowHandler for OpenWindowExample {
    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        println!("Resized: {new_size:?}");

        if let (Some(width), Some(height)) =
            (NonZeroU32::new(new_size.physical.width), NonZeroU32::new(new_size.physical.height))
        {
            self.surface.borrow_mut().resize(width, height)?;
            self.damaged.set(true);
        }

        Ok(())
    }

    fn on_frame(&self) -> Result<(), HandlerError> {
        if !self.damaged.get() {
            return Ok(());
        }

        let mut surface = self.surface.borrow_mut();
        let mut pixels = surface.buffer_mut()?;
        let size = self.window_context.size();
        let scale_factor = self.window_context.scale_factor();
        let (width, height) = (size.physical.width, size.physical.height);

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
            let mouse_pos = self.mouse_pos.get().cast::<i32>();

            let rect_x_start = (mouse_pos.x - rect_size).clamp(0, width as i32) as u32;
            let rect_x_end = (mouse_pos.x + rect_size).clamp(0, width as i32) as u32;
            let rect_y_start = (mouse_pos.y - rect_size).clamp(0, height as i32) as u32;
            let rect_y_end = (mouse_pos.y + rect_size).clamp(0, height as i32) as u32;

            for x in rect_x_start..rect_x_end {
                for y in rect_y_start..rect_y_end {
                    let index = y * width + x;
                    pixels[index as usize] = 0xFF00FF00;
                }
            }
        }

        pixels.present()?;
        self.damaged.set(false);

        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Mouse(MouseEvent::CursorMoved { position, .. }) => {
                self.mouse_pos.set(position);
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
            _ => {}
        }

        EventStatus::Captured
    }
}

impl OpenWindowExample {
    pub fn new(window: WindowContext) -> Result<Self, HandlerError> {
        let ctx = softbuffer::Context::new(window.clone())?;
        let mut surface = softbuffer::Surface::new(&ctx, window.clone())?;
        let size = window.size().physical;
        surface.resize(size.width.try_into()?, size.height.try_into()?)?;

        Ok(OpenWindowExample {
            window_context: window,
            surface: surface.into(),
            mouse_pos: PhysicalPosition::new(0., 0.).into(),
            is_cursor_inside: false.into(),
            damaged: true.into(),
        })
    }
}
