use std::collections::VecDeque;

use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

#[derive(Debug)]
pub struct Graph<'a> {
    values: &'a VecDeque<f64>,
    min: f64,
    max: f64,
    auto_scale: bool,
    auto_scale_floor: f64,
    line: Color,
    area: Color,
    frame: Color,
    guide: Color,
    capacity: usize,
}

impl<'a> Graph<'a> {
    pub fn new(values: &'a VecDeque<f64>, capacity: usize, line: Color, area: Color) -> Self {
        Self {
            values,
            min: 0.0,
            max: 1.0,
            auto_scale: false,
            auto_scale_floor: 1024.0,
            line,
            area,
            frame: Color::from_rgb8(0x33, 0x41, 0x5f),
            guide: Color::from_rgb8(0x26, 0x34, 0x4e),
            capacity,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn auto_scale(mut self, enabled: bool) -> Self {
        self.auto_scale = enabled;
        self
    }

    fn graph_max(&self) -> f64 {
        if !self.auto_scale {
            return self.max.max(self.min + 0.001);
        }
        let peak = self
            .values
            .iter()
            .copied()
            .fold(self.auto_scale_floor, f64::max);
        (peak * 1.12).max(self.min + 0.001)
    }

    fn y(&self, value: f64, height: f32, high: f64) -> f32 {
        let span = (high - self.min).max(0.001);
        let normalized = ((value - self.min) / span).clamp(0.0, 1.0) as f32;
        height - normalized * (height - 3.0) - 1.5
    }
}

impl<Message> canvas::Program<Message> for Graph<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let size = Size::new(bounds.width.max(1.0) - 1.0, bounds.height.max(1.0) - 1.0);
        frame.stroke_rectangle(
            Point::new(0.5, 0.5),
            size,
            canvas::Stroke::default()
                .with_color(self.frame)
                .with_width(1.0),
        );

        for guide in 1..4 {
            let y = (bounds.height * guide as f32 / 4.0).round() + 0.5;
            let path = canvas::Path::new(|builder| {
                builder.move_to(Point::new(1.0, y));
                builder.line_to(Point::new((bounds.width - 1.0).max(1.0), y));
            });
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(self.guide)
                    .with_width(1.0),
            );
        }

        if self.values.len() < 2 || self.capacity < 2 {
            return vec![frame.into_geometry()];
        }

        let high = self.graph_max();
        let dx = bounds.width / (self.capacity - 1) as f32;
        let x_offset = bounds.width - dx * (self.values.len() - 1) as f32;

        let area = canvas::Path::new(|builder| {
            for (index, value) in self.values.iter().copied().enumerate() {
                let point = Point::new(
                    x_offset + index as f32 * dx,
                    self.y(value, bounds.height, high),
                );
                if index == 0 {
                    builder.move_to(point);
                } else {
                    builder.line_to(point);
                }
            }
            builder.line_to(Point::new(
                x_offset + (self.values.len() - 1) as f32 * dx,
                bounds.height,
            ));
            builder.line_to(Point::new(x_offset, bounds.height));
            builder.close();
        });
        frame.fill(&area, self.area);

        let line = canvas::Path::new(|builder| {
            for (index, value) in self.values.iter().copied().enumerate() {
                let point = Point::new(
                    x_offset + index as f32 * dx,
                    self.y(value, bounds.height, high),
                );
                if index == 0 {
                    builder.move_to(point);
                } else {
                    builder.line_to(point);
                }
            }
        });
        frame.stroke(
            &line,
            canvas::Stroke::default()
                .with_color(self.line)
                .with_width(1.5),
        );

        vec![frame.into_geometry()]
    }
}
