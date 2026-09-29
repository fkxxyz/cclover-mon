#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use cclover_core::model::MonitorState;
use cclover_presentation::Dashboard;
use cclover_ui::{DashboardUi, NativeScene, Primitive, Rgba, TextAlign};

macro_rules! abi_rust_type {
    (u32) => { u32 };
    (f32) => { f32 };
    (usize) => { usize };
    (const_u8_ptr) => { *const u8 };
    (const_command_ptr) => { *const NativeCommand };
    (const_point_ptr) => { *const NativePoint };
    (poll_fn) => { unsafe extern "C" fn(*mut c_void) -> u32 };
    (scene_fn) => { unsafe extern "C" fn(*mut c_void, *mut SceneView) };
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
    frame: FrameStorage,
    quit: Option<Arc<AtomicBool>>,
}

impl NativeContext {
    pub(crate) fn new(receiver: Receiver<MonitorState>, quit: Option<Arc<AtomicBool>>) -> Self {
        let state = MonitorState::default();
        let frame = build_frame(&state);
        Self {
            receiver,
            state,
            frame,
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
            self.frame = build_frame(&self.state);
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
    width: u32,
    height: u32,
    commands: Vec<NativeCommand>,
    points: Vec<NativePoint>,
    text: Vec<Box<[u8]>>,
}

impl FrameStorage {
    fn from_scene(scene: NativeScene) -> Self {
        let mut storage = Self {
            width: scene.width,
            height: scene.height,
            commands: Vec::with_capacity(scene.primitives.len()),
            points: Vec::new(),
            text: Vec::new(),
        };
        for primitive in scene.primitives {
            storage.push(primitive);
        }
        storage
    }

    fn push(&mut self, primitive: Primitive) {
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
            } => {
                command.kind = COMMAND_FILL_RECT;
                set_rect(&mut command, rect);
                command.color = argb(color);
                command.radius = radius;
            }
            Primitive::StrokeRect {
                rect,
                color,
                width,
                radius,
            } => {
                command.kind = COMMAND_STROKE_RECT;
                set_rect(&mut command, rect);
                command.color = argb(color);
                command.stroke_width = width;
                command.radius = radius;
            }
            Primitive::Text {
                rect,
                value,
                size,
                color,
                bold,
                align,
                clip,
            } => {
                command.kind = COMMAND_TEXT;
                set_rect(&mut command, rect);
                command.color = argb(color);
                command.text_size = size;
                command.flags = (u32::from(bold) * FLAG_TEXT_BOLD)
                    | (u32::from(align == TextAlign::End) * FLAG_TEXT_END)
                    | (u32::from(clip) * FLAG_TEXT_CLIP);
                let text = value.into_bytes().into_boxed_slice();
                command.text = text.as_ptr();
                command.text_len = text.len();
                self.text.push(text);
            }
            Primitive::Polyline {
                points,
                color,
                width,
            } => {
                command.kind = COMMAND_POLYLINE;
                command.color = argb(color);
                command.stroke_width = width;
                append_points(self, &mut command, points);
            }
            Primitive::Polygon { points, color } => {
                command.kind = COMMAND_POLYGON;
                command.color = argb(color);
                append_points(self, &mut command, points);
            }
        }
        self.commands.push(command);
    }

    fn view(&self) -> SceneView {
        SceneView {
            width: self.width,
            height: self.height,
            commands: self.commands.as_ptr(),
            command_count: self.commands.len(),
            points: self.points.as_ptr(),
            point_count: self.points.len(),
        }
    }
}

fn set_rect(command: &mut NativeCommand, rect: cclover_ui::Rect) {
    command.x = rect.x;
    command.y = rect.y;
    command.width = rect.width;
    command.height = rect.height;
}

fn append_points(
    storage: &mut FrameStorage,
    command: &mut NativeCommand,
    points: Vec<cclover_ui::Point>,
) {
    command.point_offset = storage.points.len();
    command.point_count = points.len();
    storage
        .points
        .extend(points.into_iter().map(|point| NativePoint {
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

fn build_frame(state: &MonitorState) -> FrameStorage {
    let dashboard = DashboardUi::new(Dashboard::new(state));
    FrameStorage::from_scene(NativeScene::from_dashboard(&dashboard))
}

unsafe extern "C" fn poll_callback(context: *mut c_void) -> u32 {
    // SAFETY: platform hosts receive this pointer from `run` and use it only synchronously.
    let context = unsafe { &mut *(context.cast::<NativeContext>()) };
    context.poll()
}

unsafe extern "C" fn scene_callback(context: *mut c_void, scene: *mut SceneView) {
    // SAFETY: both pointers are supplied by the synchronous native host callback contract.
    let context = unsafe { &*(context.cast::<NativeContext>()) };
    unsafe { scene.write(context.frame.view()) };
}
