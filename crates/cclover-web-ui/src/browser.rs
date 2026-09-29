use std::fmt::Write as _;

use cclover_presentation::Dashboard;
use cclover_ui::{
    Block, Card, DashboardUi, Element as UiElement, GraphSpec, PANEL_GEOMETRY, PANEL_WIDTH,
    ProgressSpec, Rgba, Stack, TextCell, TextRow, TextWeight, Tone,
};
use wasm_bindgen::JsValue;
use web_sys::{Document, Element};

const ROOT_ID: &str = "cclover-root";
const STYLE_ID: &str = "cclover-style";
const SVG_NS: &str = "http://www.w3.org/2000/svg";

#[derive(Clone)]
pub struct BrowserRenderer {
    document: Document,
    root: Element,
}

impl BrowserRenderer {
    pub fn mount() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| js_error("window is unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| js_error("document is unavailable"))?;
        let body = document
            .body()
            .ok_or_else(|| js_error("document body is unavailable"))?;

        let style = if let Some(style) = document.get_element_by_id(STYLE_ID) {
            style
        } else {
            let style = document.create_element("style")?;
            style.set_id(STYLE_ID);
            body.append_child(&style)?;
            style
        };
        style.set_text_content(Some(&stylesheet()));

        let root = if let Some(root) = document.get_element_by_id(ROOT_ID) {
            root
        } else {
            let root = document.create_element("div")?;
            root.set_id(ROOT_ID);
            body.append_child(&root)?;
            root
        };

        Ok(Self { document, root })
    }

    pub fn render(&self, dashboard: Dashboard<'_>) -> Result<(), JsValue> {
        let ui = DashboardUi::new(dashboard);
        let height = ui.height();
        let available = PANEL_WIDTH - PANEL_GEOMETRY.padding * 2 - PANEL_GEOMETRY.column_spacing;
        let column_width = available as f32 / 2.0;

        let panel = self.html("div", "cclover-panel")?;
        panel.set_attribute("style", &format!("height:{height}px"))?;
        let left = self.render_column(ui.left, column_width)?;
        let right = self.render_column(ui.right, column_width)?;
        panel.append_child(&left)?;
        panel.append_child(&right)?;

        self.root.set_text_content(None);
        self.root.append_child(&panel)?;
        Ok(())
    }

    fn render_column(&self, blocks: Vec<Block<'_>>, width: f32) -> Result<Element, JsValue> {
        let column = self.html("div", "cclover-column")?;
        for block in blocks {
            let child = match block {
                Block::Section(label) => self.render_section(label)?,
                Block::Card(card) => self.render_card(card, width)?,
            };
            column.append_child(&child)?;
        }
        Ok(column)
    }

    fn render_section(&self, label: TextCell<'_>) -> Result<Element, JsValue> {
        let section = self.html("div", "cclover-section")?;
        let label = self.render_text_cell(label)?;
        section.append_child(&label)?;
        Ok(section)
    }

    fn render_card(&self, card: Card<'_>, width: f32) -> Result<Element, JsValue> {
        let Card { content, padding } = card;
        let height = padding * 2 + content.height.unwrap_or_else(|| content.content_height());
        let inner_width = (width - 2.0 * padding as f32).max(1.0);
        let card = self.html("div", "cclover-card")?;
        card.set_attribute("style", &format!("height:{height}px;padding:{padding}px"))?;
        let content = self.render_stack(content, inner_width)?;
        card.append_child(&content)?;
        Ok(card)
    }

    fn render_stack(&self, stack: Stack<'_>, width: f32) -> Result<Element, JsValue> {
        let Stack {
            children,
            gap,
            height,
        } = stack;
        let stack = self.html("div", "cclover-stack")?;
        let mut style = format!("gap:{gap}px");
        if let Some(height) = height {
            write!(style, ";height:{height}px").expect("writing to String cannot fail");
        }
        stack.set_attribute("style", &style)?;
        for child in children {
            let child = self.render_element(child, width)?;
            stack.append_child(&child)?;
        }
        Ok(stack)
    }

    fn render_element(&self, element: UiElement<'_>, width: f32) -> Result<Element, JsValue> {
        match element {
            UiElement::Row(row) => self.render_row(row),
            UiElement::Graph(graph) => self.render_graph(graph, width),
            UiElement::Progress(progress) => self.render_progress(progress),
            UiElement::Stack(stack) => self.render_stack(stack, width),
        }
    }

    fn render_row(&self, row: TextRow<'_>) -> Result<Element, JsValue> {
        let TextRow { cells, height, gap } = row;
        let row = self.html("div", "cclover-row")?;
        row.set_attribute("style", &format!("height:{height}px;gap:{gap}px"))?;
        for cell in cells {
            let cell = self.render_text_cell(cell)?;
            row.append_child(&cell)?;
        }
        Ok(row)
    }

    fn render_text_cell(&self, cell: TextCell<'_>) -> Result<Element, JsValue> {
        let text = self.html("span", "cclover-text")?;
        let mut classes = String::from("cclover-text");
        if cell.grow {
            classes.push_str(" cclover-grow");
        }
        if cell.clip {
            classes.push_str(" cclover-clip");
        }
        text.set_class_name(&classes);
        text.set_attribute(
            "style",
            &format!(
                "font-size:{}px;font-weight:{};color:{}",
                cell.size,
                match cell.weight {
                    TextWeight::Regular => 400,
                    TextWeight::Bold => 700,
                },
                css_color(cell.tone.rgba())
            ),
        )?;
        text.set_text_content(Some(cell.text.as_ref()));
        Ok(text)
    }

    fn render_progress(&self, progress: ProgressSpec) -> Result<Element, JsValue> {
        let track = self.html("div", "cclover-progress")?;
        track.set_attribute("style", &format!("height:{}px", progress.height))?;
        let fill = self.html("div", "cclover-progress-fill")?;
        fill.set_attribute(
            "style",
            &format!(
                "width:{:.4}%;background:{}",
                progress.value.clamp(0.0, 1.0) * 100.0,
                css_color(progress.tone.rgba())
            ),
        )?;
        track.append_child(&fill)?;
        Ok(track)
    }

    fn render_graph(&self, graph: GraphSpec<'_>, width: f32) -> Result<Element, JsValue> {
        let height = graph.height as f32;
        let svg = self.svg("svg")?;
        svg.set_attribute("class", "cclover-graph")?;
        svg.set_attribute("viewBox", &format!("0 0 {width:.3} {height:.3}"))?;
        svg.set_attribute("preserveAspectRatio", "none")?;
        svg.set_attribute("style", &format!("height:{}px", graph.height))?;

        let frame = self.svg("rect")?;
        frame.set_attribute("x", "0.5")?;
        frame.set_attribute("y", "0.5")?;
        frame.set_attribute("width", &format!("{:.3}", (width - 1.0).max(0.0)))?;
        frame.set_attribute("height", &format!("{:.3}", (height - 1.0).max(0.0)))?;
        frame.set_attribute("fill", "none")?;
        frame.set_attribute("stroke", &css_color(Tone::Border.rgba()))?;
        frame.set_attribute("stroke-width", "1")?;
        frame.set_attribute("vector-effect", "non-scaling-stroke")?;
        svg.append_child(&frame)?;

        for guide in 1..4 {
            let y = (height * guide as f32 / 4.0).round() + 0.5;
            let line = self.svg("line")?;
            line.set_attribute("x1", "1")?;
            line.set_attribute("x2", &format!("{:.3}", (width - 1.0).max(1.0)))?;
            line.set_attribute("y1", &format!("{y:.3}"))?;
            line.set_attribute("y2", &format!("{y:.3}"))?;
            line.set_attribute("stroke", &css_color(Tone::Guide.rgba()))?;
            line.set_attribute("stroke-width", "1")?;
            line.set_attribute("vector-effect", "non-scaling-stroke")?;
            svg.append_child(&line)?;
        }

        if graph.values.len() < 2 || graph.capacity < 2 {
            return Ok(svg);
        }

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
            write!(line_points, "{x:.3},{y:.3} ").expect("writing to String cannot fail");
        }

        let mut area_points = line_points.clone();
        write!(
            area_points,
            "{last_x:.3},{height:.3} {first_x:.3},{height:.3}"
        )
        .expect("writing to String cannot fail");
        let area = self.svg("polygon")?;
        area.set_attribute("points", &area_points)?;
        let mut fill = graph.line.rgba();
        fill.a = graph.fill_alpha;
        area.set_attribute("fill", &css_color(fill))?;
        svg.append_child(&area)?;

        let line = self.svg("polyline")?;
        line.set_attribute("points", &line_points)?;
        line.set_attribute("fill", "none")?;
        line.set_attribute("stroke", &css_color(graph.line.rgba()))?;
        line.set_attribute("stroke-width", "1.5")?;
        line.set_attribute("vector-effect", "non-scaling-stroke")?;
        svg.append_child(&line)?;
        Ok(svg)
    }

    fn html(&self, tag: &str, class: &str) -> Result<Element, JsValue> {
        let element = self.document.create_element(tag)?;
        element.set_class_name(class);
        Ok(element)
    }

    fn svg(&self, tag: &str) -> Result<Element, JsValue> {
        self.document.create_element_ns(Some(SVG_NS), tag)
    }
}

fn stylesheet() -> String {
    format!(
        r#"
html, body {{ margin: 0; padding: 0; background: transparent; overflow: hidden; }}
body {{ width: {panel_width}px; min-width: {panel_width}px; font-family: "Fira Sans", "Segoe UI", sans-serif; }}
.cclover-panel, .cclover-panel * {{ box-sizing: border-box; }}
.cclover-panel {{
  width: {panel_width}px;
  min-height: {min_height}px;
  padding: {padding}px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: {column_gap}px;
  background: {background};
  box-shadow: inset 0 0 0 1px {border};
  border-radius: 16px;
  overflow: hidden;
}}
.cclover-column {{ display: flex; flex-direction: column; gap: {column_gap}px; min-width: 0; }}
.cclover-section {{ height: {section_height}px; display: flex; align-items: center; min-width: 0; overflow: hidden; }}
.cclover-card {{ width: 100%; background: {card}; box-shadow: inset 0 0 0 1px {border}; border-radius: 12px; overflow: hidden; }}
.cclover-stack {{ width: 100%; display: flex; flex-direction: column; min-width: 0; }}
.cclover-row {{ width: 100%; display: flex; align-items: center; min-width: 0; overflow: hidden; }}
.cclover-text {{ flex: 0 0 auto; min-width: 0; line-height: 1; white-space: nowrap; text-align: right; }}
.cclover-grow {{ flex: 1 1 0; text-align: left; }}
.cclover-clip {{ overflow: hidden; }}
.cclover-progress {{ width: 100%; background: {border}; border-radius: 3px; overflow: hidden; }}
.cclover-progress-fill {{ height: 100%; border-radius: 3px; }}
.cclover-graph {{ display: block; width: 100%; flex: none; overflow: hidden; }}
"#,
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
    format!(
        "rgba({}, {}, {}, {:.3})",
        color.r, color.g, color.b, color.a
    )
}

fn js_error(message: &str) -> JsValue {
    JsValue::from_str(message)
}
