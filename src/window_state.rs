use crate::MouseCursor;

/// The struct that is passed to the user's application. This is akin to winit's `Window` struct
#[derive(Debug)]
pub struct WindowState {
    width: u32,
    height: u32,
    scale: f64,
    mouse_cursor: MouseCursor,
    raw_handle: raw_window_handle::RawWindowHandle,
    resized: bool,
    cursor_requested: bool,
    redraw_requested: bool,
    close_requested: bool,
}

impl WindowState {
    pub fn new(
        width: u32,
        height: u32,
        scale: f64,
        raw_handle: raw_window_handle::RawWindowHandle,
    ) -> Self {
        Self {
            width,
            height,
            scale,
            raw_handle,
            mouse_cursor: Default::default(),
            resized: false,
            cursor_requested: false,
            redraw_requested: true,
            close_requested: false,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.resized = true;
        }
    }

    pub fn request_cursor(&mut self, cursor: MouseCursor) {
        if self.mouse_cursor != cursor {
            self.mouse_cursor = cursor;
            self.cursor_requested = true;
        }
    }

    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    pub fn request_close(&mut self) {
        self.close_requested = true;
    }

    pub fn poll_cursor_request(&mut self) -> Option<MouseCursor> {
        if self.cursor_requested {
            self.cursor_requested = false;
            Some(self.mouse_cursor)
        } else {
            None
        }
    }

    pub fn poll_redraw_request(&mut self) -> bool {
        if self.redraw_requested {
            self.redraw_requested = false;
            true
        } else {
            false
        }
    }

    pub fn poll_close_request(&mut self) -> bool {
        if self.close_requested {
            self.close_requested = false;
            true
        } else {
            false
        }
    }
}

unsafe impl raw_window_handle::HasRawWindowHandle for WindowState {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.raw_handle
    }
}
