#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use cclover_core::model::MonitorState;
use cclover_presentation::Dashboard;
use cclover_ui::{
    CacheClass, DashboardUi, NativeScene, NativeTextMeasurer, Primitive, Rect, Rgba, TextAlign,
    TextWeight,
};

type MeasureTextFn = unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32) -> f32;

macro_rules! abi_rust_type {
    (u32) => { u32 };
    (u64) => { u64 };
    (f32) => { f32 };
    (usize) => { usize };
    (const_u8_ptr) => { *const u8 };
    (const_command_ptr) => { *const NativeCommand };
    (const_point_ptr) => { *const NativePoint };
    (const_damage_rect_ptr) => { *const NativeDamageRect };
    (poll_fn) => { unsafe extern "C" fn(*mut c_void) -> u32 };
    (scene_fn) => {
        unsafe extern "C" fn(*mut c_void, *mut c_void, MeasureTextFn, *mut SceneView)
    };
}

macro_rules! define_native_abi {
    (
        constants {
            $( $rust_const:ident => $c_const:ident = $value:expr; )*
        }
        structs {
            $(
                $rust_struct:ident => $c_struct:ident {
                    $( $field:ident : $field_type:ident, )*
                }
            )*
        }
    ) => {
        $( pub(crate) const $rust_const: u32 = $value; )*

        $(
            #[repr(C)]
            #[derive(Clone, Copy)]
            pub(crate) struct $rust_struct {
                $( pub(crate) $field: abi_rust_type!($field_type), )*
            }
        )*
    };
}

include!("native_abi_spec.rs");

pub struct DesktopApp {
    pub(crate) receiver: Receiver<MonitorState>,
}

impl DesktopApp {
    pub fn new(receiver: Receiver<MonitorState>) -> Self {
        Self { receiver }
    }
}

pub(crate) struct NativeContext {
    receiver: Receiver<MonitorState>,
    state: MonitorState,
    frame: Option<FrameStorage>,
    frame_dirty: bool,
    static_revision: u64,
    quit: Option<Arc<AtomicBool>>,
}

impl NativeContext {
    pub(crate) fn new(receiver: Receiver<MonitorState>, quit: Option<Arc<AtomicBool>>) -> Self {
        let state = MonitorState::default();
        Self {
            receiver,
            state,
            frame: None,
            frame_dirty: true,
            static_revision: 0,
            quit,
        }
    }

    fn poll(&mut self) -> u32 {
        let mut flags = 0;
        if self
            .quit
            .as_ref()
            .is_some_and(|quit| quit.load(Ordering::Relaxed))
        {
            flags |= POLL_QUIT;
        }

        let mut latest = None;
        while let Ok(state) = self.receiver.try_recv() {
            latest = Some(state);
        }
        if let Some(state) = latest {
            self.state = state;
            self.frame_dirty = true;
            flags |= POLL_FRAME;
        }
        flags
    }

    pub(crate) fn as_ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast::<c_void>()
    }
}

impl HostCallbacks {
    pub(crate) const fn new() -> Self {
        Self {
            poll: poll_callback,
            scene: scene_callback,
        }
    }
}

struct FrameStorage {
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

    fn view(&self) -> SceneView {
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

struct HostTextMeasurer {
    context: *mut c_void,
    measure: MeasureTextFn,
}

impl NativeTextMeasurer for HostTextMeasurer {
    fn width(&self, text: &str, size: u32, weight: TextWeight) -> f32 {
        let flags = u32::from(weight == TextWeight::Bold) * FLAG_TEXT_BOLD;
        // SAFETY: the native host supplies this callback and context for the duration of the
        // synchronous scene request. The UTF-8 byte slice remains alive for the call.
        unsafe { (self.measure)(self.context, text.as_ptr(), text.len(), size, flags) }
    }
}

fn build_frame(
    state: &MonitorState,
    text: &impl NativeTextMeasurer,
    previous: Option<&NativeScene>,
    previous_static_revision: u64,
) -> FrameStorage {
    let dashboard = DashboardUi::new(Dashboard::new(state));
    let scene = NativeScene::from_dashboard(&dashboard, text);
    FrameStorage::from_scene(scene, previous, previous_static_revision)
}

unsafe extern "C" fn poll_callback(context: *mut c_void) -> u32 {
    // SAFETY: platform hosts receive this pointer from `run` and use it only synchronously.
    let context = unsafe { &mut *(context.cast::<NativeContext>()) };
    context.poll()
}

unsafe extern "C" fn scene_callback(
    context: *mut c_void,
    measure_context: *mut c_void,
    measure_text: MeasureTextFn,
    scene: *mut SceneView,
) {
    // SAFETY: pointers and callback are supplied by the synchronous native host contract.
    let context = unsafe { &mut *(context.cast::<NativeContext>()) };
    if context.frame_dirty || context.frame.is_none() {
        let text = HostTextMeasurer {
            context: measure_context,
            measure: measure_text,
        };
        let frame = build_frame(
            &context.state,
            &text,
            context.frame.as_ref().map(|frame| &frame.scene),
            context.static_revision,
        );
        context.static_revision = frame.static_revision;
        context.frame = Some(frame);
        context.frame_dirty = false;
    }
    let frame = context
        .frame
        .as_ref()
        .expect("scene frame must be initialized");
    unsafe { scene.write(frame.view()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(r: u8) -> Rgba {
        Rgba {
            r,
            g: 0,
            b: 0,
            a: 1.0,
        }
    }

    fn fill(x: f32, color: Rgba, static_content: bool) -> Primitive {
        Primitive::FillRect {
            rect: Rect {
                x,
                y: 10.0,
                width: 10.0,
                height: 10.0,
            },
            color,
            radius: 0.0,
            static_content,
        }
    }

    fn scene(primitives: Vec<Primitive>) -> NativeScene {
        NativeScene {
            width: 100,
            height: 100,
            primitives,
        }
    }

    #[test]
    fn unchanged_scene_has_no_damage_and_keeps_static_revision() {
        let previous = scene(vec![
            fill(10.0, color(1), true),
            fill(20.0, color(2), false),
        ]);
        let current = previous.clone();

        let invalidation = scene_invalidation(Some(&previous), &current, 7);

        assert!(!invalidation.full_redraw);
        assert_eq!(invalidation.static_revision, 7);
        assert!(invalidation.damage_rects.is_empty());
    }

    #[test]
    fn dynamic_change_damages_union_of_old_and_new_bounds() {
        let previous = scene(vec![
            fill(10.0, color(1), true),
            fill(10.0, color(2), false),
        ]);
        let current = scene(vec![
            fill(10.0, color(1), true),
            fill(20.0, color(2), false),
        ]);

        let invalidation = scene_invalidation(Some(&previous), &current, 7);

        assert!(!invalidation.full_redraw);
        assert_eq!(invalidation.static_revision, 7);
        assert_eq!(invalidation.damage_rects.len(), 1);
        let damage = invalidation.damage_rects[0];
        assert_eq!(
            (damage.x1, damage.y1, damage.x2, damage.y2),
            (9.0, 9.0, 31.0, 21.0)
        );
    }

    #[test]
    fn static_change_advances_revision_and_forces_full_redraw() {
        let previous = scene(vec![
            fill(10.0, color(1), true),
            fill(20.0, color(2), false),
        ]);
        let current = scene(vec![
            fill(10.0, color(3), true),
            fill(20.0, color(2), false),
        ]);

        let invalidation = scene_invalidation(Some(&previous), &current, 7);

        assert!(invalidation.full_redraw);
        assert_eq!(invalidation.static_revision, 8);
        assert!(invalidation.damage_rects.is_empty());
    }

    #[test]
    fn dynamic_primitive_count_change_falls_back_to_full_redraw() {
        let previous = scene(vec![
            fill(10.0, color(1), true),
            fill(20.0, color(2), false),
        ]);
        let current = scene(vec![
            fill(10.0, color(1), true),
            fill(20.0, color(2), false),
            fill(40.0, color(3), false),
        ]);

        let invalidation = scene_invalidation(Some(&previous), &current, 7);

        assert!(invalidation.full_redraw);
        assert_eq!(invalidation.static_revision, 7);
        assert!(invalidation.damage_rects.is_empty());
    }
}
