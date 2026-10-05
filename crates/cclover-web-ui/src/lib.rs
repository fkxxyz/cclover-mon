use std::fmt::Write as _;

use cclover_ui::{PANEL_WIDTH, Primitive, Rect, Rgba, Scene, TextAlign};

pub fn render(scene: &Scene) -> String {
    let mut out = String::with_capacity(24 * 1024);
    write!(
        out,
        "<svg class=\"cclover-panel\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" xmlns=\"http://www.w3.org/2000/svg\">",
        scene.width, scene.height, scene.width, scene.height
    )
    .unwrap();

    out.push_str("<defs>");
    for (index, primitive) in scene.primitives.iter().enumerate() {
        if let Primitive::Text {
            rect, clip: true, ..
        } = primitive
        {
            write!(
                out,
                "<clipPath id=\"clip-{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                number(rect.x),
                number(rect.y),
                number(rect.width),
                number(rect.height)
            )
            .unwrap();
        }
    }
    out.push_str("</defs>");

    for (index, primitive) in scene.primitives.iter().enumerate() {
        render_primitive(&mut out, primitive, index);
    }
    out.push_str("</svg>");
    out
}

fn render_primitive(out: &mut String, primitive: &Primitive, index: usize) {
    match primitive {
        Primitive::FillRect {
            rect,
            color,
            radius,
            ..
        } => render_rect(out, *rect, Some(*color), None, 0.0, *radius),
        Primitive::StrokeRect {
            rect,
            color,
            width,
            radius,
            ..
        } => render_rect(out, *rect, None, Some(*color), *width, *radius),
        Primitive::Text {
            rect,
            value,
            size,
            color,
            bold,
            align,
            clip,
            must_fit,
            ..
        } => {
            let x = match align {
                TextAlign::Start => rect.x,
                TextAlign::End => rect.x + rect.width,
            };
            write!(
                out,
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"{}\" fill=\"{}\" text-anchor=\"{}\" dominant-baseline=\"middle\"",
                number(x),
                number(rect.y + rect.height / 2.0),
                size,
                if *bold { 700 } else { 400 },
                css_color(*color),
                match align {
                    TextAlign::Start => "start",
                    TextAlign::End => "end",
                }
            )
            .unwrap();
            if *clip {
                write!(out, " clip-path=\"url(#clip-{index})\"").unwrap();
            }
            if *must_fit {
                write!(
                    out,
                    " data-cclover-must-fit=\"1\" data-cclover-max-width=\"{}\"",
                    number(rect.width)
                )
                .unwrap();
            }
            out.push('>');
            escape_xml(out, value);
            out.push_str("</text>");
        }
        Primitive::Polyline {
            points,
            color,
            width,
            ..
        } => {
            out.push_str("<polyline points=\"");
            render_points(out, points);
            write!(
                out,
                "\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" vector-effect=\"non-scaling-stroke\"/>",
                css_color(*color),
                number(*width)
            )
            .unwrap();
        }
        Primitive::Polygon { points, color } => {
            out.push_str("<polygon points=\"");
            render_points(out, points);
            write!(out, "\" fill=\"{}\"/>", css_color(*color)).unwrap();
        }
    }
}

fn render_rect(
    out: &mut String,
    rect: Rect,
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    stroke_width: f32,
    radius: f32,
) {
    write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"",
        number(rect.x),
        number(rect.y),
        number(rect.width),
        number(rect.height),
        number(radius)
    )
    .unwrap();
    match fill {
        Some(color) => write!(out, " fill=\"{}\"", css_color(color)).unwrap(),
        None => out.push_str(" fill=\"none\""),
    }
    if let Some(color) = stroke {
        write!(
            out,
            " stroke=\"{}\" stroke-width=\"{}\" vector-effect=\"non-scaling-stroke\"",
            css_color(color),
            number(stroke_width)
        )
        .unwrap();
    }
    out.push_str("/>");
}

fn render_points(out: &mut String, points: &[cclover_ui::Point]) {
    for (index, point) in points.iter().enumerate() {
        if index != 0 {
            out.push(' ');
        }
        write!(out, "{},{}", number(point.x), number(point.y)).unwrap();
    }
}

pub fn stylesheet() -> String {
    format!(
        "html,body{{margin:0;padding:0;background:transparent;overflow:hidden}}body{{width:{PANEL_WIDTH}px;min-width:{PANEL_WIDTH}px;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}}.cclover-panel{{display:block}}"
    )
}

fn css_color(color: Rgba) -> String {
    format!("rgba({},{},{},{:.3})", color.r, color.g, color.b, color.a)
}

fn number(value: f32) -> String {
    let mut value = format!("{value:.3}");
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    value
}

fn escape_xml(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cclover_core::model::MonitorState;
    use cclover_presentation::Dashboard;

    #[test]
    fn renders_shared_scene_as_svg_without_layout_css() {
        let state = MonitorState::default();
        let scene = cclover_ui::build_scene(Dashboard::new(&state));
        let svg = render(&scene);
        assert!(svg.starts_with("<svg class=\"cclover-panel\""));
        assert!(svg.contains("<text"));
        assert!(svg.contains("<rect"));
        let css = stylesheet();
        assert!(!css.contains("display:grid"));
        assert!(!css.contains("display:flex"));
    }

    #[test]
    fn preserves_scene_geometry_in_svg() {
        let scene = Scene {
            width: 100,
            height: 50,
            primitives: vec![Primitive::FillRect {
                rect: Rect {
                    x: 10.0,
                    y: 11.5,
                    width: 30.0,
                    height: 20.0,
                },
                color: Rgba {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 1.0,
                },
                radius: 4.0,
                static_content: true,
            }],
        };
        let svg = render(&scene);
        assert!(svg.contains("x=\"10\" y=\"11.5\" width=\"30\" height=\"20\" rx=\"4\""));
    }

    #[test]
    fn emits_must_fit_metadata_without_changing_scene_geometry() {
        let scene = Scene {
            width: 100,
            height: 50,
            primitives: vec![Primitive::Text {
                rect: Rect {
                    x: 10.0,
                    y: 10.0,
                    width: 37.5,
                    height: 20.0,
                },
                value: "99.9%".to_owned(),
                size: 12,
                color: Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 1.0,
                },
                bold: true,
                align: TextAlign::End,
                clip: false,
                must_fit: true,
                static_content: false,
            }],
        };

        let svg = render(&scene);

        assert!(svg.contains("data-cclover-must-fit=\"1\""));
        assert!(svg.contains("data-cclover-max-width=\"37.5\""));
        assert!(svg.contains("x=\"47.5\""));
    }

    #[test]
    fn escapes_dynamic_text() {
        let mut out = String::new();
        escape_xml(&mut out, "<x & \"y\">");
        assert_eq!(out, "&lt;x &amp; &quot;y&quot;&gt;");
    }
}
