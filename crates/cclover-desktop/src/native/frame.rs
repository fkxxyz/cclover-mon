use std::ptr;

use cclover_core::model::MonitorState;
use cclover_ui::{CacheClass, Primitive, Rect, Rgba, Scene, TextAlign, build_scene};

use super::abi::*;

pub(super) struct FrameStorage {
    scene: Scene,
    static_revision: u64,
    full_redraw: bool,
    damage_rects: Vec<NativeDamageRect>,
    redraw_mask: Vec<u8>,
    commands: Vec<NativeCommand>,
    points: Vec<NativePoint>,
}

impl FrameStorage {
    fn from_scene(scene: Scene, previous: Option<&Scene>, previous_static_revision: u64) -> Self {
        let invalidation = scene_invalidation(previous, &scene, previous_static_revision);
        let damage_rects = invalidation
            .damage_rects
            .iter()
            .copied()
            .map(native_damage_rect)
            .collect();
        let mut commands = Vec::with_capacity(scene.primitives.len());
        let mut points = Vec::new();
        let mut redraw_mask = Vec::new();
        for primitive in &scene.primitives {
            let command_start = commands.len();
            push_command(&mut commands, &mut points, primitive);
            if !invalidation.full_redraw {
                let redraw = primitive.cache_class() == CacheClass::Dynamic
                    && invalidation
                        .damage_rects
                        .iter()
                        .any(|damage| rects_intersect_or_touch(primitive.damage_bounds(), *damage));
                redraw_mask.resize(commands.len(), u8::from(redraw));
                debug_assert!(commands.len() > command_start);
            }
        }
        debug_assert!(invalidation.full_redraw || redraw_mask.len() == commands.len());
        Self {
            scene,
            static_revision: invalidation.static_revision,
            full_redraw: invalidation.full_redraw,
            damage_rects,
            redraw_mask,
            commands,
            points,
        }
    }

    pub(super) fn scene(&self) -> &Scene {
        &self.scene
    }

    pub(super) fn static_revision(&self) -> u64 {
        self.static_revision
    }

    pub(super) fn view(&self) -> SceneView {
        SceneView {
            width: self.scene.width,
            height: self.scene.height,
            static_revision: self.static_revision,
            full_redraw: u32::from(self.full_redraw),
            damage_rects: if self.damage_rects.is_empty() {
                ptr::null()
            } else {
                self.damage_rects.as_ptr()
            },
            damage_count: self.damage_rects.len(),
            redraw_mask: if self.redraw_mask.is_empty() {
                ptr::null()
            } else {
                self.redraw_mask.as_ptr()
            },
            redraw_mask_count: self.redraw_mask.len(),
            commands: self.commands.as_ptr(),
            command_count: self.commands.len(),
            points: self.points.as_ptr(),
            point_count: self.points.len(),
        }
    }
}

struct SceneInvalidation {
    static_revision: u64,
    full_redraw: bool,
    damage_rects: Vec<Rect>,
}

fn scene_invalidation(
    previous: Option<&Scene>,
    current: &Scene,
    previous_static_revision: u64,
) -> SceneInvalidation {
    let dimensions_changed = previous.is_some_and(|previous| {
        previous.width != current.width || previous.height != current.height
    });
    let static_changed = previous.is_none_or(|previous| {
        dimensions_changed || !same_class_primitives(previous, current, CacheClass::Static)
    });
    let static_revision = if static_changed {
        previous_static_revision.wrapping_add(1).max(1)
    } else {
        previous_static_revision
    };
    let mut full_redraw = previous.is_none() || dimensions_changed || static_changed;
    let mut damage = Vec::new();

    if !full_redraw {
        let previous = previous.expect("previous scene exists when diffing dynamic primitives");
        let mut old = previous
            .primitives
            .iter()
            .filter(|primitive| primitive.cache_class() == CacheClass::Dynamic);
        let mut new = current
            .primitives
            .iter()
            .filter(|primitive| primitive.cache_class() == CacheClass::Dynamic);
        loop {
            match (old.next(), new.next()) {
                (None, None) => break,
                (Some(old), Some(new)) => {
                    if old != new {
                        add_damage(
                            &mut damage,
                            union_rect(old.damage_bounds(), new.damage_bounds()),
                        );
                    }
                }
                _ => {
                    full_redraw = true;
                    damage.clear();
                    break;
                }
            }
        }
    }

    SceneInvalidation {
        static_revision,
        full_redraw,
        damage_rects: damage,
    }
}

fn same_class_primitives(a: &Scene, b: &Scene, class: CacheClass) -> bool {
    a.primitives
        .iter()
        .filter(|primitive| primitive.cache_class() == class)
        .eq(b
            .primitives
            .iter()
            .filter(|primitive| primitive.cache_class() == class))
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.min(b.x);
    let y1 = a.y.min(b.y);
    let x2 = (a.x + a.width).max(b.x + b.width);
    let y2 = (a.y + a.height).max(b.y + b.height);
    Rect {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    }
}

fn rects_intersect_or_touch(a: Rect, b: Rect) -> bool {
    a.x <= b.x + b.width && a.x + a.width >= b.x && a.y <= b.y + b.height && a.y + a.height >= b.y
}

fn add_damage(rects: &mut Vec<Rect>, mut rect: Rect) {
    let mut index = 0;
    while index < rects.len() {
        if rects_intersect_or_touch(rects[index], rect) {
            rect = union_rect(rects.swap_remove(index), rect);
            index = 0;
        } else {
            index += 1;
        }
    }
    rects.push(rect);
}

fn native_damage_rect(rect: Rect) -> NativeDamageRect {
    NativeDamageRect {
        x1: rect.x,
        y1: rect.y,
        x2: rect.x + rect.width,
        y2: rect.y + rect.height,
    }
}

fn push_command(
    commands: &mut Vec<NativeCommand>,
    points: &mut Vec<NativePoint>,
    primitive: &Primitive,
) {
    let mut command = NativeCommand {
        kind: 0,
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        color: 0,
        radius: 0.0,
        stroke_width: 0.0,
        point_offset: 0,
        point_count: 0,
        text: ptr::null(),
        text_len: 0,
        text_size: 0,
        flags: 0,
    };
    match primitive {
        Primitive::FillRect {
            rect,
            color,
            radius,
            ..
        } => {
            command.kind = COMMAND_FILL_RECT;
            set_rect(&mut command, *rect);
            command.color = argb(*color);
            command.radius = *radius;
        }
        Primitive::StrokeRect {
            rect,
            color,
            width,
            radius,
            ..
        } => {
            command.kind = COMMAND_STROKE_RECT;
            set_rect(&mut command, *rect);
            command.color = argb(*color);
            command.stroke_width = *width;
            command.radius = *radius;
        }
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
            command.kind = COMMAND_TEXT;
            set_rect(&mut command, *rect);
            command.color = argb(*color);
            command.text_size = *size;
            command.flags = (u32::from(*bold) * FLAG_TEXT_BOLD)
                | (u32::from(*align == TextAlign::End) * FLAG_TEXT_END)
                | (u32::from(*clip) * FLAG_TEXT_CLIP)
                | (u32::from(*must_fit) * FLAG_TEXT_MUST_FIT);
            command.text = value.as_ptr();
            command.text_len = value.len();
        }
        Primitive::Polyline {
            points: source,
            color,
            width,
            ..
        } => {
            command.kind = COMMAND_POLYLINE;
            command.color = argb(*color);
            command.stroke_width = *width;
            append_points(points, &mut command, source);
        }
        Primitive::Polygon {
            points: source,
            color,
        } => {
            command.kind = COMMAND_POLYGON;
            command.color = argb(*color);
            append_points(points, &mut command, source);
        }
    }
    if primitive.cache_class() == CacheClass::Static {
        command.flags |= FLAG_STATIC_CONTENT;
    }
    commands.push(command);
}

fn set_rect(command: &mut NativeCommand, rect: cclover_ui::Rect) {
    command.x = rect.x;
    command.y = rect.y;
    command.width = rect.width;
    command.height = rect.height;
}

fn append_points(
    storage: &mut Vec<NativePoint>,
    command: &mut NativeCommand,
    points: &[cclover_ui::Point],
) {
    command.point_offset = storage.len();
    command.point_count = points.len();
    storage.extend(points.iter().map(|point| NativePoint {
        x: point.x,
        y: point.y,
    }));
}

fn argb(color: Rgba) -> u32 {
    ((color.a.clamp(0.0, 1.0) * 255.0).round() as u32) << 24
        | u32::from(color.r) << 16
        | u32::from(color.g) << 8
        | u32::from(color.b)
}

pub(super) fn build_frame(
    state: &MonitorState,
    previous: Option<&Scene>,
    previous_static_revision: u64,
) -> FrameStorage {
    let scene = build_scene(cclover_presentation::Dashboard::new(state));
    FrameStorage::from_scene(scene, previous, previous_static_revision)
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "linux"))]
mod cairo_tests;
