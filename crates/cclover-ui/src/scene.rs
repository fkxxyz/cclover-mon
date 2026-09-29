use crate::{
    Block, Card, DashboardUi, Element, GraphSpec, PANEL_GEOMETRY, PANEL_WIDTH, Rgba, Stack,
    TextRow, TextWeight, Tone,
};
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
pub enum TextAlign {
    Start,
    End,
}

pub trait NativeTextMeasurer {
    fn width(&self, text: &str, size: u32, weight: TextWeight) -> f32;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    FillRect {
        rect: Rect,
        color: Rgba,
        radius: f32,
    },
    StrokeRect {
        rect: Rect,
        color: Rgba,
        width: f32,
        radius: f32,
    },
    Text {
        rect: Rect,
        value: String,
        size: u32,
        color: Rgba,
        bold: bool,
        align: TextAlign,
        clip: bool,
    },
    Polyline {
        points: Vec<Point>,
        color: Rgba,
        width: f32,
    },
    Polygon {
        points: Vec<Point>,
        color: Rgba,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeScene {
    pub width: u32,
    pub height: u32,
    pub primitives: Vec<Primitive>,
}

impl NativeScene {
    pub fn from_dashboard(ui: &DashboardUi<'_>, text: &impl NativeTextMeasurer) -> Self {
        let height = ui.height();
        let mut primitives = Vec::new();
        let panel = Rect {
            x: 0.5,
            y: 0.5,
            width: PANEL_WIDTH as f32 - 1.0,
            height: height as f32 - 1.0,
        };
        primitives.push(Primitive::FillRect {
            rect: panel,
            color: Tone::Background.rgba(),
            radius: 16.0,
        });
        primitives.push(Primitive::StrokeRect {
            rect: panel,
            color: Tone::Border.rgba(),
            width: 1.0,
            radius: 16.0,
        });
        let available = PANEL_WIDTH - PANEL_GEOMETRY.padding * 2 - PANEL_GEOMETRY.column_spacing;
        let column_width = available as f32 / 2.0;
        let left_x = PANEL_GEOMETRY.padding as f32;
        let right_x = left_x + column_width + PANEL_GEOMETRY.column_spacing as f32;
        lower_column(&ui.left, left_x, column_width, text, &mut primitives);
        lower_column(&ui.right, right_x, column_width, text, &mut primitives);
        Self {
            width: PANEL_WIDTH,
            height,
            primitives,
        }
    }
}

fn lower_column(
    blocks: &[Block<'_>],
    x: f32,
    width: f32,
    text: &impl NativeTextMeasurer,
    out: &mut Vec<Primitive>,
) {
    let mut y = PANEL_GEOMETRY.padding as f32;
    for block in blocks {
        match block {
            Block::Section(label) => out.push(Primitive::Text {
                rect: Rect {
                    x,
                    y,
                    width,
                    height: PANEL_GEOMETRY.section_height as f32,
                },
                value: label.text.to_string(),
                size: label.size,
                color: label.tone.rgba(),
                bold: label.weight == TextWeight::Bold,
                align: TextAlign::Start,
                clip: label.clip,
            }),
            Block::Card(card) => lower_card(card, x, y, width, text, out),
        }
        y += block.height() as f32 + PANEL_GEOMETRY.column_spacing as f32;
    }
}

fn lower_card(
    card: &Card<'_>,
    x: f32,
    y: f32,
    width: f32,
    text: &impl NativeTextMeasurer,
    out: &mut Vec<Primitive>,
) {
    let rect = Rect {
        x,
        y,
        width,
        height: card.height() as f32,
    };
    out.push(Primitive::FillRect {
        rect,
        color: Tone::Card.rgba(),
        radius: 12.0,
    });
    out.push(Primitive::StrokeRect {
        rect,
        color: Tone::Border.rgba(),
        width: 1.0,
        radius: 12.0,
    });
    let p = card.padding as f32;
    lower_stack(
        &card.content,
        x + p,
        y + p,
        (width - 2.0 * p).max(0.0),
        text,
        out,
    );
}

fn lower_stack(
    stack: &Stack<'_>,
    x: f32,
    y: f32,
    width: f32,
    text: &impl NativeTextMeasurer,
    out: &mut Vec<Primitive>,
) {
    let mut child_y = y;
    for child in &stack.children {
        match child {
            Element::Row(row) => lower_row(row, x, child_y, width, text, out),
            Element::Graph(graph) => lower_graph(graph, x, child_y, width, out),
            Element::Progress(progress) => {
                let h = progress.height as f32;
                out.push(Primitive::FillRect {
                    rect: Rect {
                        x,
                        y: child_y,
                        width,
                        height: h,
                    },
                    color: Tone::Border.rgba(),
                    radius: 3.0,
                });
                out.push(Primitive::FillRect {
                    rect: Rect {
                        x,
                        y: child_y,
                        width: width * progress.value,
                        height: h,
                    },
                    color: progress.tone.rgba(),
                    radius: 3.0,
                });
            }
            Element::Stack(nested) => lower_stack(nested, x, child_y, width, text, out),
        }
        child_y += child.height() as f32 + stack.gap as f32;
    }
}

fn lower_row(
    row: &TextRow<'_>,
    x: f32,
    y: f32,
    width: f32,
    text: &impl NativeTextMeasurer,
    out: &mut Vec<Primitive>,
) {
    if row.cells.is_empty() {
        return;
    }
    let gaps = row.gap as f32 * row.cells.len().saturating_sub(1) as f32;
    let fixed = row
        .cells
        .iter()
        .filter(|cell| !cell.grow)
        .map(|cell| text.width(&cell.text, cell.size, cell.weight))
        .sum::<f32>();
    let growers = row.cells.iter().filter(|cell| cell.grow).count();
    let flexible = (width - gaps - fixed).max(0.0) / growers.max(1) as f32;
    let mut cell_x = x;
    for cell in &row.cells {
        let cell_width = if cell.grow {
            flexible
        } else {
            text.width(&cell.text, cell.size, cell.weight)
        };
        if !cell.text.is_empty() {
            out.push(Primitive::Text {
                rect: Rect {
                    x: cell_x,
                    y,
                    width: cell_width,
                    height: row.height as f32,
                },
                value: cell.text.to_string(),
                size: cell.size,
                color: cell.tone.rgba(),
                bold: cell.weight == TextWeight::Bold,
                align: if cell.grow {
                    TextAlign::Start
                } else {
                    TextAlign::End
                },
                clip: cell.clip,
            });
        }
        cell_x += cell_width + row.gap as f32;
    }
}

fn lower_graph(graph: &GraphSpec<'_>, x: f32, y: f32, width: f32, out: &mut Vec<Primitive>) {
    let height = graph.height as f32;
    out.push(Primitive::StrokeRect {
        rect: Rect {
            x,
            y,
            width,
            height,
        },
        color: Tone::Border.rgba(),
        width: 1.0,
        radius: 0.0,
    });
    for guide in 1..4 {
        let gy = y + (height * guide as f32 / 4.0).round() + 0.5;
        out.push(Primitive::Polyline {
            points: vec![
                Point { x: x + 1.0, y: gy },
                Point {
                    x: x + width - 1.0,
                    y: gy,
                },
            ],
            color: Tone::Guide.rgba(),
            width: 1.0,
        });
    }
    if graph.values.len() < 2 || graph.capacity < 2 {
        return;
    }
    let high = if graph.auto_scale {
        (graph.values.iter().copied().fold(1024.0_f64, f64::max) * 1.12).max(graph.min + 0.001)
    } else {
        graph.max.max(graph.min + 0.001)
    };
    let span = (high - graph.min).max(0.001);
    let dx = width / (graph.capacity - 1) as f32;
    let x_offset = x + width - dx * (graph.values.len() - 1) as f32;
    let mut line = Vec::with_capacity(graph.values.len());
    for (index, value) in graph.values.iter().copied().enumerate() {
        let normalized = ((value - graph.min) / span).clamp(0.0, 1.0) as f32;
        line.push(Point {
            x: x_offset + index as f32 * dx,
            y: y + height - normalized * (height - 3.0) - 1.5,
        });
    }
    let mut area = line.clone();
    area.push(Point {
        x: line.last().expect("line non-empty").x,
        y: y + height,
    });
    area.push(Point {
        x: line[0].x,
        y: y + height,
    });
    let mut fill = graph.line.rgba();
    fill.a = graph.fill_alpha;
    out.push(Primitive::Polygon {
        points: area,
        color: fill,
    });
    out.push(Primitive::Polyline {
        points: line,
        color: graph.line.rgba(),
        width: 1.5,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cclover_core::model::MonitorState;
    use cclover_presentation::Dashboard;

    struct TestMeasurer;

    impl NativeTextMeasurer for TestMeasurer {
        fn width(&self, text: &str, size: u32, _weight: TextWeight) -> f32 {
            text.chars().count() as f32 * size as f32 * 0.5
        }
    }

    #[test]
    fn native_scene_is_derived_from_shared_dashboard_tree() {
        let state = MonitorState::default();
        let ui = DashboardUi::new(Dashboard::new(&state));
        let scene = NativeScene::from_dashboard(&ui, &TestMeasurer);
        assert_eq!(scene.width, PANEL_WIDTH);
        assert_eq!(scene.height, ui.height());
        assert!(scene.primitives.iter().any(
            |primitive| matches!(primitive, Primitive::Text { value, .. } if value == "MEMORY")
        ));
    }

    #[test]
    fn native_row_uses_renderer_measurement_for_natural_width_cells() {
        struct FixedMeasurer;

        impl NativeTextMeasurer for FixedMeasurer {
            fn width(&self, _text: &str, _size: u32, _weight: TextWeight) -> f32 {
                37.5
            }
        }

        let row = TextRow {
            cells: vec![
                crate::TextCell {
                    text: "grow".into(),
                    size: 11,
                    tone: Tone::Foreground,
                    weight: TextWeight::Regular,
                    grow: true,
                    clip: false,
                },
                crate::TextCell {
                    text: "fixed".into(),
                    size: 11,
                    tone: Tone::Foreground,
                    weight: TextWeight::Bold,
                    grow: false,
                    clip: false,
                },
            ],
            height: 16,
            gap: 4,
        };
        let mut primitives = Vec::new();
        lower_row(&row, 10.0, 20.0, 100.0, &FixedMeasurer, &mut primitives);

        let text_rects = primitives
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Text { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(text_rects.len(), 2);
        assert_eq!(text_rects[0].width, 58.5);
        assert_eq!(text_rects[1].x, 72.5);
        assert_eq!(text_rects[1].width, 37.5);
    }
}
