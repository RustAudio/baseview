#![allow(unused)]

use femtovg::{Align, Baseline, Canvas, Color, Paint, Path, Renderer};
use std::cell::Cell;

const HISTORY_COUNT: usize = 300;

pub struct PerfGraph {
    values: Vec<Cell<f32>>,
    head: Cell<usize>,
}

impl PerfGraph {
    pub fn new() -> Self {
        Self { values: vec![0.0.into(); HISTORY_COUNT], head: Default::default() }
    }

    pub fn update(&self, frame_time: f32) {
        self.head.set((self.head.get() + 1) % HISTORY_COUNT);
        self.values[self.head.get()].set(frame_time);
    }

    pub fn get_average(&self) -> f32 {
        self.values.iter().map(|f| f.get()).sum::<f32>() / HISTORY_COUNT as f32
    }

    pub fn render<T: Renderer>(&self, canvas: &mut Canvas<T>, x: f32, y: f32) {
        let avg = self.get_average();

        let w = 200.0;
        let h = 35.0;

        let mut path = Path::new();
        path.rect(x, y, w, h);
        canvas.fill_path(&path, &Paint::color(Color::rgba(0, 0, 0, 128)));

        let mut path = Path::new();
        path.move_to(x, y + h);

        for i in 0..HISTORY_COUNT {
            let mut v = 1.0 / (0.00001 + self.values[(self.head.get() + i) % HISTORY_COUNT].get());
            if v > 80.0 {
                v = 80.0;
            }
            let vx = x + (i as f32 / (HISTORY_COUNT - 1) as f32) * w;
            let vy = y + h - ((v / 80.0) * h);
            path.line_to(vx, vy);
        }

        path.line_to(x + w, y + h);
        canvas.fill_path(&path, &Paint::color(Color::rgba(255, 192, 0, 128)));

        let text_paint = Paint::color(Color::rgba(240, 240, 240, 255)).with_font_size(12.0);
        let _ = canvas.fill_text(x + 5.0, y + 13.0, "Frame time", &text_paint);

        let text_paint = Paint::color(Color::rgba(240, 240, 240, 255))
            .with_font_size(14.0)
            .with_text_align(Align::Right)
            .with_text_baseline(Baseline::Top);
        let _ = canvas.fill_text(x + w - 5.0, y, format!("{:.2} FPS", 1.0 / avg), &text_paint);

        let text_paint = Paint::color(Color::rgba(240, 240, 240, 200))
            .with_font_size(12.0)
            .with_text_align(Align::Right)
            .with_text_baseline(Baseline::Alphabetic);
        let _ = canvas.fill_text(
            x + w - 5.0,
            y + h - 5.0,
            format!("{:.2} ms", avg * 1000.0),
            &text_paint,
        );
    }
}
