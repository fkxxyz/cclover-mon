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
            write!(out, "<clipPath id=\"clip-{index}\"><rect x=\"").unwrap();
            write_number(&mut out, rect.x);
            out.push_str("\" y=\"");
            write_number(&mut out, rect.y);
            out.push_str("\" width=\"");
            write_number(&mut out, rect.width);
            out.push_str("\" height=\"");
            write_number(&mut out, rect.height);
            out.push_str("\"/></clipPath>");
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
            out.push_str("<text x=\"");
            write_number(out, x);
            out.push_str("\" y=\"");
            write_number(out, rect.y + rect.height / 2.0);
            write!(
                out,
                "\" font-size=\"{}\" font-weight=\"{}\" fill=\"{}\" text-anchor=\"{}\" dominant-baseline=\"middle\"",
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
                out.push_str(" data-cclover-must-fit=\"1\" data-cclover-max-width=\"");
                write_number(out, rect.width);
                out.push('"');
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
                "\" fill=\"none\" stroke=\"{}\" stroke-width=\"",
                css_color(*color)
            )
            .unwrap();
            write_number(out, *width);
            out.push_str("\" vector-effect=\"non-scaling-stroke\"/>");
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
    out.push_str("<rect x=\"");
    write_number(out, rect.x);
    out.push_str("\" y=\"");
    write_number(out, rect.y);
    out.push_str("\" width=\"");
    write_number(out, rect.width);
    out.push_str("\" height=\"");
    write_number(out, rect.height);
    out.push_str("\" rx=\"");
    write_number(out, radius);
    out.push('"');
    match fill {
        Some(color) => write!(out, " fill=\"{}\"", css_color(color)).unwrap(),
        None => out.push_str(" fill=\"none\""),
    }
    if let Some(color) = stroke {
        write!(out, " stroke=\"{}\" stroke-width=\"", css_color(color)).unwrap();
        write_number(out, stroke_width);
        out.push_str("\" vector-effect=\"non-scaling-stroke\"");
    }
    out.push_str("/>");
}

fn render_points(out: &mut String, points: &[cclover_ui::Point]) {
    for (index, point) in points.iter().enumerate() {
        if index != 0 {
            out.push(' ');
        }
        write_number(out, point.x);
        out.push(',');
        write_number(out, point.y);
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

fn write_number(out: &mut String, value: f32) {
    let start = out.len();
    write!(out, "{value:.3}").unwrap();
    let has_fraction = out.as_bytes()[start..].contains(&b'.');
    if !has_fraction {
        return;
    }

    while out.len() > start && out.as_bytes().last() == Some(&b'0') {
        out.pop();
    }
    if out.len() > start && out.as_bytes().last() == Some(&b'.') {
        out.pop();
    }
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

    #[test]
    fn writes_scene_numbers_with_existing_svg_formatting() {
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "-0"),
            (1.0, "1"),
            (1.2, "1.2"),
            (1.23, "1.23"),
            (1.234, "1.234"),
            (1.2346, "1.235"),
            (-1.5, "-1.5"),
        ] {
            let mut out = String::from("prefix=");
            write_number(&mut out, value);
            assert_eq!(out, format!("prefix={expected}"), "value={value:?}");
        }
    }
}
