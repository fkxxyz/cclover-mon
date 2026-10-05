use cclover_presentation::Dashboard;

use crate::{
    Block, Card, CellWidth, DashboardUi, Element, GraphSpec, PANEL_GEOMETRY, PANEL_WIDTH, Point,
    Primitive, Rect, Scene, Stack, TextRow, TextWeight, Tone,
};

pub fn build_scene(dashboard: Dashboard<'_>) -> Scene {
    let ui = DashboardUi::new(dashboard);
    layout_dashboard(&ui)
}

pub fn layout_dashboard(ui: &DashboardUi<'_>) -> Scene {
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
        static_content: true,
    });
    primitives.push(Primitive::StrokeRect {
        rect: panel,
        color: Tone::Border.rgba(),
        width: 1.0,
        radius: 16.0,
        static_content: true,
    });
    let available = PANEL_WIDTH - PANEL_GEOMETRY.padding * 2 - PANEL_GEOMETRY.column_spacing;
    let column_width = available as f32 / 2.0;
    let left_x = PANEL_GEOMETRY.padding as f32;
    let right_x = left_x + column_width + PANEL_GEOMETRY.column_spacing as f32;
    lower_column(&ui.left, left_x, column_width, &mut primitives);
    lower_column(&ui.right, right_x, column_width, &mut primitives);
    Scene {
        width: PANEL_WIDTH,
        height,
        primitives,
    }
}

fn lower_column(blocks: &[Block<'_>], x: f32, width: f32, out: &mut Vec<Primitive>) {
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
                align: label.align,
                clip: label.clip,
                must_fit: label.must_fit,
                static_content: true,
            }),
            Block::Card(card) => lower_card(card, x, y, width, out),
        }
        y += block.height() as f32 + PANEL_GEOMETRY.column_spacing as f32;
    }
}

fn lower_card(card: &Card<'_>, x: f32, y: f32, width: f32, out: &mut Vec<Primitive>) {
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
        static_content: true,
    });
    out.push(Primitive::StrokeRect {
        rect,
        color: Tone::Border.rgba(),
        width: 1.0,
        radius: 12.0,
        static_content: true,
    });
    let p = card.padding as f32;
    lower_stack(&card.content, x + p, y + p, (width - 2.0 * p).max(0.0), out);
}

fn lower_stack(stack: &Stack<'_>, x: f32, y: f32, width: f32, out: &mut Vec<Primitive>) {
    let mut child_y = y;
    for child in &stack.children {
        match child {
            Element::Row(row) => lower_row(row, x, child_y, width, out),
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
                    static_content: true,
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
                    static_content: false,
                });
            }
            Element::Stack(nested) => lower_stack(nested, x, child_y, width, out),
        }
        child_y += child.height() as f32 + stack.gap as f32;
    }
}

fn lower_row(row: &TextRow<'_>, x: f32, y: f32, width: f32, out: &mut Vec<Primitive>) {
    if row.cells.is_empty() {
        return;
    }
    let gaps = row.gap as f32 * row.cells.len().saturating_sub(1) as f32;
    let fixed = row
        .cells
        .iter()
        .map(|cell| match cell.width {
            CellWidth::Fixed(width) => width as f32,
            CellWidth::Fill => 0.0,
        })
        .sum::<f32>();
    let fills = row
        .cells
        .iter()
        .filter(|cell| cell.width == CellWidth::Fill)
        .count();
    let fill_width = if fills == 0 {
        0.0
    } else {
        (width - gaps - fixed).max(0.0) / fills as f32
    };
    let mut cell_x = x;
    for cell in &row.cells {
        let cell_width = match cell.width {
            CellWidth::Fixed(width) => width as f32,
            CellWidth::Fill => fill_width,
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
                align: cell.align,
                clip: cell.clip,
                must_fit: cell.must_fit,
                static_content: cell.static_content,
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
        static_content: true,
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
            static_content: true,
        });
    }
    if graph.values.len() < 2 || graph.capacity < 2 {
        return;
    }
    let high = graph.resolved_max();
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
        static_content: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cclover_core::model::MonitorState;

    #[test]
    fn scene_is_derived_from_shared_dashboard_tree() {
        let state = MonitorState::default();
        let ui = DashboardUi::new(Dashboard::new(&state));
        let scene = layout_dashboard(&ui);
        assert_eq!(scene.width, PANEL_WIDTH);
        assert_eq!(scene.height, ui.height());
        assert!(scene.primitives.iter().any(
            |primitive| matches!(primitive, Primitive::Text { value, .. } if value == "MEMORY")
        ));
    }

    #[test]
    fn bounded_values_lower_to_explicit_must_fit_text() {
        let state = MonitorState::default();
        let scene = build_scene(Dashboard::new(&state));
        let bounded = scene
            .primitives
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Text {
                    must_fit: true,
                    clip,
                    rect,
                    ..
                } => Some((*clip, *rect)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            !bounded.is_empty(),
            "default dashboard must contain bounded values"
        );
        assert!(
            bounded.iter().all(|(clip, rect)| !clip && rect.width > 0.0),
            "bounded values must fit their authoritative slot rather than rely on clipping"
        );
    }

    #[test]
    fn row_geometry_does_not_depend_on_text_metrics() {
        let row = TextRow {
            cells: vec![
                crate::TextCell {
                    text: "fill".into(),
                    size: 11,
                    tone: Tone::Foreground,
                    weight: TextWeight::Regular,
                    width: CellWidth::Fill,
                    align: crate::TextAlign::Start,
                    clip: true,
                    must_fit: false,
                    static_content: false,
                },
                crate::TextCell {
                    text: "fixed".into(),
                    size: 11,
                    tone: Tone::Foreground,
                    weight: TextWeight::Bold,
                    width: CellWidth::Fixed(38),
                    align: crate::TextAlign::End,
                    clip: true,
                    must_fit: false,
                    static_content: false,
                },
            ],
            height: 16,
            gap: 4,
        };
        let mut primitives = Vec::new();
        lower_row(&row, 10.0, 20.0, 100.0, &mut primitives);
        let rects = primitives
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Text { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(rects[0].width, 58.0);
        assert_eq!(rects[1].x, 72.0);
        assert_eq!(rects[1].width, 38.0);
    }
}
