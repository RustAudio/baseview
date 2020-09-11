use crate::MouseCursor;

// The struct that is passed to the user's application. This is akin to winit's `Window` struct

/// The current state of the window
#[derive(Debug)]
pub struct WindowState {
    width: u32,
    height: u32,
    scale: f64,
    mouse_cursor: MouseCursor,
    raw_handle: raw_window_handle::RawWindowHandle,
    frame_rate: f64,
    interval_rate: Option<f64>,
    size_requested: bool,
    cursor_requested: bool,
    redraw_requested: bool,
    close_requested: bool,
    frame_rate_requested: bool,
    interval_requested: bool,
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
            size_requested: false,
            frame_rate: 60.0,
            interval_rate: None,
            cursor_requested: false,
            redraw_requested: true,
            close_requested: false,
            frame_rate_requested: false,
            interval_requested: false,
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

    pub fn mouse_cursor(&self) -> MouseCursor {
        self.mouse_cursor
    }

    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    pub fn interval_rate(&self) -> Option<f64> {
        self.interval_rate
    }

    pub fn request_size(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.size_requested = true;
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

    /// Request the rate at which `draw()` is called in frames per second.
    ///
    /// Note that `request_redraw()` must be called after an update for `draw()` to be called.
    ///
    /// By default this is `60.0`.
    pub fn request_frame_rate(&mut self, frame_rate: f64) {
        if self.frame_rate != frame_rate {
            self.frame_rate = frame_rate;
            self.frame_rate_requested = true;
        }
    }

    /// Request the rate at which the `Interval` event is called in calls per second.
    /// Set this to `None` if do not want this event.
    ///
    /// By default this is `None`.
    pub fn request_interval(&mut self, interval_rate: Option<f64>) {
        if self.interval_rate != interval_rate {
            self.interval_rate = interval_rate;
            self.interval_requested = true;
        }
    }

    pub fn poll_size_request(&mut self) -> Option<(u32, u32)> {
        if self.size_requested {
            self.size_requested = false;
            Some((self.width, self.height))
        } else {
            None
        }
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

    pub fn poll_frame_rate_request(&mut self) -> Option<f64> {
        if self.frame_rate_requested {
            self.frame_rate_requested = false;
            Some(self.frame_rate)
        } else {
            None
        }
    }

    pub fn poll_interval_request(&mut self) -> Option<Option<f64>> {
        if self.interval_requested {
            self.interval_requested = false;
            Some(self.interval_rate)
        } else {
            None
        }
    }
}

unsafe impl raw_window_handle::HasRawWindowHandle for WindowState {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.raw_handle
    }
}
