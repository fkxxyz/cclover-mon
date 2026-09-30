use std::fmt::Write as _;

use cclover_presentation::Dashboard;
use cclover_ui::{
    Block, Card, DashboardUi, Element, GraphSpec, PANEL_GEOMETRY, PANEL_WIDTH, ProgressSpec, Rgba,
    Stack, TextCell, TextRow, TextWeight, Tone,
};

pub fn render(dashboard: Dashboard<'_>) -> String {
    let ui = DashboardUi::new(dashboard);
    let height = ui.height();
    let available = PANEL_WIDTH - PANEL_GEOMETRY.padding * 2 - PANEL_GEOMETRY.column_spacing;
    let column_width = available as f32 / 2.0;
    let mut out = String::with_capacity(16 * 1024);
    write!(
        out,
        "<div class=\"cclover-panel\" style=\"height:{height}px\">"
    )
    .unwrap();
    render_column(&mut out, ui.left, column_width);
    render_column(&mut out, ui.right, column_width);
    out.push_str("</div>");
    out
}

fn render_column(out: &mut String, blocks: Vec<Block<'_>>, width: f32) {
    out.push_str("<div class=\"cclover-column\">");
    for block in blocks {
        match block {
            Block::Section(label) => {
                out.push_str("<div class=\"cclover-section\">");
                render_text_cell(out, label);
                out.push_str("</div>");
            }
            Block::Card(card) => render_card(out, card, width),
        }
    }
    out.push_str("</div>");
}

fn render_card(out: &mut String, card: Card<'_>, width: f32) {
    let Card { content, padding } = card;
    let height = padding * 2 + content.height.unwrap_or_else(|| content.content_height());
    let inner_width = (width - 2.0 * padding as f32).max(1.0);
    write!(
        out,
        "<div class=\"cclover-card\" style=\"height:{height}px;padding:{padding}px\">"
    )
    .unwrap();
    render_stack(out, content, inner_width);
    out.push_str("</div>");
}

fn render_stack(out: &mut String, stack: Stack<'_>, width: f32) {
    let Stack {
        children,
        gap,
        height,
    } = stack;
    write!(out, "<div class=\"cclover-stack\" style=\"gap:{gap}px").unwrap();
    if let Some(height) = height {
        write!(out, ";height:{height}px").unwrap();
    }
    out.push_str("\">");
    for child in children {
        render_element(out, child, width);
    }
    out.push_str("</div>");
}

fn render_element(out: &mut String, element: Element<'_>, width: f32) {
    match element {
        Element::Row(row) => render_row(out, row),
        Element::Graph(graph) => render_graph(out, graph, width),
        Element::Progress(progress) => render_progress(out, progress),
        Element::Stack(stack) => render_stack(out, stack, width),
    }
}

fn render_row(out: &mut String, row: TextRow<'_>) {
    let TextRow { cells, height, gap } = row;
    write!(
        out,
        "<div class=\"cclover-row\" style=\"height:{height}px;gap:{gap}px\">"
    )
    .unwrap();
    for cell in cells {
        render_text_cell(out, cell);
    }
    out.push_str("</div>");
}

fn render_text_cell(out: &mut String, cell: TextCell<'_>) {
    out.push_str("<span class=\"cclover-text");
    if cell.grow {
        out.push_str(" cclover-grow");
    }
    if cell.clip {
        out.push_str(" cclover-clip");
    }
    write!(
        out,
        "\" style=\"font-size:{}px;font-weight:{};color:{}\">",
        cell.size,
        match cell.weight {
            TextWeight::Regular => 400,
            TextWeight::Bold => 700,
        },
        css_color(cell.tone.rgba())
    )
    .unwrap();
    escape_html(out, cell.text.as_ref());
    out.push_str("</span>");
}

fn render_progress(out: &mut String, progress: ProgressSpec) {
    write!(
        out,
        "<div class=\"cclover-progress\" style=\"height:{}px\"><div class=\"cclover-progress-fill\" style=\"width:{:.4}%;background:{}\"></div></div>",
        progress.height,
        progress.value.clamp(0.0, 1.0) * 100.0,
        css_color(progress.tone.rgba())
    )
    .unwrap();
}

fn render_graph(out: &mut String, graph: GraphSpec<'_>, width: f32) {
    let height = graph.height as f32;
    write!(
        out,
        "<svg class=\"cclover-graph\" viewBox=\"0 0 {width:.3} {height:.3}\" preserveAspectRatio=\"none\" style=\"height:{}px\" xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"0.5\" y=\"0.5\" width=\"{:.3}\" height=\"{:.3}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1\" vector-effect=\"non-scaling-stroke\"/>",
        graph.height,
        (width - 1.0).max(0.0),
        (height - 1.0).max(0.0),
        css_color(Tone::Border.rgba())
    )
    .unwrap();

    for guide in 1..4 {
        let y = (height * guide as f32 / 4.0).round() + 0.5;
        write!(
            out,
            "<line x1=\"1\" x2=\"{:.3}\" y1=\"{y:.3}\" y2=\"{y:.3}\" stroke=\"{}\" stroke-width=\"1\" vector-effect=\"non-scaling-stroke\"/>",
            (width - 1.0).max(1.0),
            css_color(Tone::Guide.rgba())
        )
        .unwrap();
    }

    if graph.values.len() >= 2 && graph.capacity >= 2 {
        let high = graph.resolved_max();
        let span = (high - graph.min).max(0.001);
        let dx = width / (graph.capacity - 1) as f32;
        let x_offset = width - dx * (graph.values.len() - 1) as f32;
        let mut line_points = String::new();
        let mut first_x = 0.0;
        let mut last_x = 0.0;
        for (index, value) in graph.values.iter().copied().enumerate() {
            let x = x_offset + index as f32 * dx;
            let normalized = ((value - graph.min) / span).clamp(0.0, 1.0) as f32;
            let y = height - normalized * (height - 3.0) - 1.5;
            if index == 0 {
                first_x = x;
            }
            last_x = x;
            write!(line_points, "{x:.3},{y:.3} ").unwrap();
        }
        let mut fill = graph.line.rgba();
        fill.a = graph.fill_alpha;
        write!(
            out,
            "<polygon points=\"{}{last_x:.3},{height:.3} {first_x:.3},{height:.3}\" fill=\"{}\"/><polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\" vector-effect=\"non-scaling-stroke\"/>",
            line_points,
            css_color(fill),
            line_points,
            css_color(graph.line.rgba())
        )
        .unwrap();
    }
    out.push_str("</svg>");
}

pub fn stylesheet() -> String {
    format!(
        r#"html,body{{margin:0;padding:0;background:transparent;overflow:hidden}}body{{width:{panel_width}px;min-width:{panel_width}px;font-family:"Fira Sans","Segoe UI",sans-serif}}.cclover-panel,.cclover-panel *{{box-sizing:border-box}}.cclover-panel{{width:{panel_width}px;min-height:{min_height}px;padding:{padding}px;display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:{column_gap}px;background:{background};box-shadow:inset 0 0 0 1px {border};border-radius:16px;overflow:hidden}}.cclover-column{{display:flex;flex-direction:column;gap:{column_gap}px;min-width:0}}.cclover-section{{height:{section_height}px;display:flex;align-items:center;min-width:0;overflow:hidden}}.cclover-card{{width:100%;background:{card};box-shadow:inset 0 0 0 1px {border};border-radius:12px;overflow:hidden}}.cclover-stack{{width:100%;display:flex;flex-direction:column;min-width:0}}.cclover-row{{width:100%;display:flex;align-items:center;min-width:0;overflow:hidden}}.cclover-text{{flex:0 0 auto;min-width:0;line-height:1;white-space:nowrap;text-align:right}}.cclover-grow{{flex:1 1 0;text-align:left}}.cclover-clip{{overflow:hidden}}.cclover-progress{{width:100%;background:{border};border-radius:3px;overflow:hidden}}.cclover-progress-fill{{height:100%;border-radius:3px}}.cclover-graph{{display:block;width:100%;flex:none;overflow:hidden}}"#,
        panel_width = PANEL_WIDTH,
        min_height = PANEL_GEOMETRY.min_height,
        padding = PANEL_GEOMETRY.padding,
        column_gap = PANEL_GEOMETRY.column_spacing,
        section_height = PANEL_GEOMETRY.section_height,
        background = css_color(Tone::Background.rgba()),
        card = css_color(Tone::Card.rgba()),
        border = css_color(Tone::Border.rgba()),
    )
}

fn css_color(color: Rgba) -> String {
    format!("rgba({},{},{},{:.3})", color.r, color.g, color.b, color.a)
}

fn escape_html(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cclover_core::model::MonitorState;

    #[test]
    fn renders_dashboard_as_html_without_browser_runtime() {
        let state = MonitorState::default();
        let html = render(Dashboard::new(&state));
        assert!(html.starts_with("<div class=\"cclover-panel\""));
        assert!(html.contains("<svg"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn escapes_dynamic_text() {
        let mut out = String::new();
        escape_html(&mut out, "<x & \"y\">");
        assert_eq!(out, "&lt;x &amp; &quot;y&quot;&gt;");
    }
}
