use crate::{Rgba, TextAlign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheClass {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    FillRect {
        rect: Rect,
        color: Rgba,
        radius: f32,
        static_content: bool,
    },
    StrokeRect {
        rect: Rect,
        color: Rgba,
        width: f32,
        radius: f32,
        static_content: bool,
    },
    Text {
        rect: Rect,
        value: String,
        size: u32,
        color: Rgba,
        bold: bool,
        align: TextAlign,
        clip: bool,
        must_fit: bool,
        static_content: bool,
    },
    Polyline {
        points: Vec<Point>,
        color: Rgba,
        width: f32,
        static_content: bool,
    },
    Polygon {
        points: Vec<Point>,
        color: Rgba,
    },
}

impl Primitive {
    pub fn cache_class(&self) -> CacheClass {
        match self {
            Self::FillRect { static_content, .. }
            | Self::StrokeRect { static_content, .. }
            | Self::Text { static_content, .. }
            | Self::Polyline { static_content, .. } => {
                if *static_content {
                    CacheClass::Static
                } else {
                    CacheClass::Dynamic
                }
            }
            Self::Polygon { .. } => CacheClass::Dynamic,
        }
    }

    pub fn damage_bounds(&self) -> Rect {
        let (mut bounds, expand) = match self {
            Self::FillRect { rect, .. } => (*rect, 1.0),
            Self::StrokeRect { rect, width, .. } => (*rect, *width + 1.0),
            Self::Text { rect, clip, .. } => (*rect, if *clip { 0.0 } else { 3.0 }),
            Self::Polyline { points, width, .. } => (point_bounds(points), *width + 1.0),
            Self::Polygon { points, .. } => (point_bounds(points), 1.0),
        };
        bounds.x -= expand;
        bounds.y -= expand;
        bounds.width += expand * 2.0;
        bounds.height += expand * 2.0;
        bounds
    }
}

fn point_bounds(points: &[Point]) -> Rect {
    let Some(first) = points.first() else {
        return Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    };
    let (mut x1, mut y1, mut x2, mut y2) = (first.x, first.y, first.x, first.y);
    for point in &points[1..] {
        x1 = x1.min(point.x);
        y1 = y1.min(point.y);
        x2 = x2.max(point.x);
        y2 = y2.max(point.y);
    }
    Rect {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub primitives: Vec<Primitive>,
}
