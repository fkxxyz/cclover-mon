use std::{ffi::c_void, ptr};

use cclover_core::model::MonitorState;
use cclover_presentation::Dashboard;
use cclover_ui::{
    CacheClass, DashboardUi, NativeScene, NativeTextMeasurer, Primitive, Rect, Rgba, TextAlign,
    TextWeight,
};

use super::abi::*;

pub(super) struct FrameStorage {
    scene: NativeScene,
    static_revision: u64,
    full_redraw: bool,
    damage_rects: Vec<NativeDamageRect>,
    commands: Vec<NativeCommand>,
    points: Vec<NativePoint>,
}

impl FrameStorage {
    fn from_scene(
        scene: NativeScene,
        previous: Option<&NativeScene>,
        previous_static_revision: u64,
    ) -> Self {
        let invalidation = scene_invalidation(previous, &scene, previous_static_revision);
        let mut commands = Vec::with_capacity(scene.primitives.len());
        let mut points = Vec::new();
        for primitive in &scene.primitives {
            push_command(&mut commands, &mut points, primitive);
        }
        Self {
            scene,
            static_revision: invalidation.static_revision,
            full_redraw: invalidation.full_redraw,
            damage_rects: invalidation.damage_rects,
            commands,
            points,
        }
    }

    pub(super) fn scene(&self) -> &NativeScene {
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
    damage_rects: Vec<NativeDamageRect>,
}

fn scene_invalidation(
    previous: Option<&NativeScene>,
    current: &NativeScene,
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
        damage_rects: damage.into_iter().map(native_damage_rect).collect(),
    }
}

fn same_class_primitives(a: &NativeScene, b: &NativeScene, class: CacheClass) -> bool {
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

fn rects_touch(a: Rect, b: Rect) -> bool {
    a.x <= b.x + b.width && a.x + a.width >= b.x && a.y <= b.y + b.height && a.y + a.height >= b.y
}

fn add_damage(rects: &mut Vec<Rect>, mut rect: Rect) {
    let mut index = 0;
    while index < rects.len() {
        if rects_touch(rects[index], rect) {
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
            ..
        } => {
            command.kind = COMMAND_TEXT;
            set_rect(&mut command, *rect);
            command.color = argb(*color);
            command.text_size = *size;
            command.flags = (u32::from(*bold) * FLAG_TEXT_BOLD)
                | (u32::from(*align == TextAlign::End) * FLAG_TEXT_END)
                | (u32::from(*clip) * FLAG_TEXT_CLIP);
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

pub(super) struct HostTextMeasurer {
    pub(super) context: *mut c_void,
    pub(super) measure: MeasureTextFn,
}

impl NativeTextMeasurer for HostTextMeasurer {
    fn width(&self, text: &str, size: u32, weight: TextWeight) -> f32 {
        let flags = u32::from(weight == TextWeight::Bold) * FLAG_TEXT_BOLD;
        // SAFETY: the native host supplies this callback and context for the duration of the
        // synchronous scene request. The UTF-8 byte slice remains alive for the call.
        unsafe { (self.measure)(self.context, text.as_ptr(), text.len(), size, flags) }
    }
}

pub(super) fn build_frame(
    state: &MonitorState,
    text: &impl NativeTextMeasurer,
    previous: Option<&NativeScene>,
    previous_static_revision: u64,
) -> FrameStorage {
    let dashboard = DashboardUi::new(Dashboard::new(state));
    let scene = NativeScene::from_dashboard(&dashboard, text);
    FrameStorage::from_scene(scene, previous, previous_static_revision)
}

#[cfg(test)]
mod tests;
