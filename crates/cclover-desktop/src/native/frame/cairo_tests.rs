use std::ffi::c_void;

use super::*;

unsafe extern "C" {
    fn cairo_image_surface_create_for_data(
        data: *mut u8,
        format: i32,
        width: i32,
        height: i32,
        stride: i32,
    ) -> *mut c_void;
    fn cairo_surface_flush(surface: *mut c_void);
    fn cairo_surface_mark_dirty_rectangle(
        surface: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    );
    fn cairo_surface_destroy(surface: *mut c_void);
    fn cairo_create(surface: *mut c_void) -> *mut c_void;
    fn cairo_destroy(cr: *mut c_void);
    fn cairo_save(cr: *mut c_void);
    fn cairo_restore(cr: *mut c_void);
    fn cairo_rectangle(cr: *mut c_void, x: f64, y: f64, width: f64, height: f64);
    fn cairo_clip(cr: *mut c_void);
    fn cclover_cairo_configure_context(cr: *mut c_void);
    fn cclover_cairo_validate_scene(
        cr: *mut c_void,
        scene: *const SceneView,
        mode: i32,
        cull_to_redraw_mask: i32,
    ) -> i32;
    fn cclover_cairo_execute_validated_scene(
        renderer: *mut c_void,
        cr: *mut c_void,
        scene: *const SceneView,
        clear: i32,
        mode: i32,
        cull_to_redraw_mask: i32,
    );
}

const CAIRO_FORMAT_ARGB32: i32 = 0;
const CAIRO_DRAW_ALL: i32 = 0;
const CAIRO_DRAW_STATIC: i32 = 1;
const CAIRO_DRAW_DYNAMIC: i32 = 2;

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

fn scene(primitives: Vec<Primitive>) -> Scene {
    Scene {
        width: 100,
        height: 100,
        primitives,
    }
}

#[test]
fn production_cairo_incremental_culling_matches_full_redraw_for_overlapping_shapes() {
    let background = Primitive::FillRect {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
        },
        color: color(1),
        radius: 0.0,
        static_content: true,
    };
    let first_scene = cairo_shape_scene(background.clone(), 10.0, color(20), 60.0);
    let second_scene = cairo_shape_scene(background.clone(), 20.0, color(20), 60.0);
    let mut inserted_scene = second_scene.clone();
    inserted_scene
        .primitives
        .insert(2, fill(45.0, color(30), false));
    let removed_scene = cairo_shape_scene(background, 5.0, color(50), 64.0);
    let scenes = [first_scene, second_scene, inserted_scene, removed_scene];

    let first = FrameStorage::from_scene(scenes[0].clone(), None, 0);
    let mut incremental = CairoImage::new(first.scene.width as i32, first.scene.height as i32);
    cairo_render(&mut incremental, &first, CAIRO_DRAW_ALL, false, true);
    let mut static_layer = CairoImage::new(first.scene.width as i32, first.scene.height as i32);
    cairo_render(&mut static_layer, &first, CAIRO_DRAW_STATIC, false, true);
    let mut previous = &scenes[0];
    let mut revision = first.static_revision;

    for current in &scenes[1..] {
        let frame = FrameStorage::from_scene(current.clone(), Some(previous), revision);
        assert!(!frame.full_redraw);
        cairo_restore_damage(&mut incremental, &mut static_layer, &frame.damage_rects);
        cairo_render_incremental(&mut incremental, &frame);

        let mut expected = CairoImage::new(frame.scene.width as i32, frame.scene.height as i32);
        cairo_render(&mut expected, &frame, CAIRO_DRAW_ALL, false, true);
        assert_eq!(
            incremental.pixels(),
            expected.pixels(),
            "production Cairo incremental culling must remain pixel-equivalent to full redraw"
        );
        revision = frame.static_revision;
        previous = current;
    }
}

#[test]
fn cairo_incremental_validation_uses_the_same_redraw_mask_as_execution() {
    let previous = scene(vec![
        must_fit_text(0.0, 1.0, "999.9%"),
        must_fit_text(50.0, 40.0, "10%"),
    ]);
    let current = scene(vec![
        must_fit_text(0.0, 1.0, "999.9%"),
        must_fit_text(50.0, 40.0, "11%"),
    ]);
    let frame = FrameStorage::from_scene(current, Some(&previous), 7);
    assert_eq!(frame.redraw_mask, vec![0, 1]);

    let image = CairoImage::new(frame.scene.width as i32, frame.scene.height as i32);
    let view = frame.view();
    assert_eq!(
        unsafe { cclover_cairo_validate_scene(image.cr, &view, CAIRO_DRAW_DYNAMIC, 1) },
        1,
        "incremental validation must skip unchanged text that execution will not draw"
    );
    assert_eq!(
        unsafe { cclover_cairo_validate_scene(image.cr, &view, CAIRO_DRAW_DYNAMIC, 0) },
        0,
        "full dynamic validation must still reject the unchanged overflowing text"
    );

    let previous = scene(vec![
        must_fit_text(0.0, 40.0, "10%"),
        must_fit_text(50.0, 40.0, "10%"),
    ]);
    let current = scene(vec![
        must_fit_text(0.0, 40.0, "10%"),
        must_fit_text(50.0, 1.0, "999.9%"),
    ]);
    let frame = FrameStorage::from_scene(current, Some(&previous), 7);
    assert_eq!(frame.redraw_mask, vec![0, 1]);
    let view = frame.view();
    assert_eq!(
        unsafe { cclover_cairo_validate_scene(image.cr, &view, CAIRO_DRAW_DYNAMIC, 1) },
        0,
        "incremental validation must reject overflowing text selected for execution"
    );
}

fn cairo_shape_scene(background: Primitive, x: f32, fill_color: Rgba, line_y: f32) -> Scene {
    scene(vec![
        background,
        fill(x, fill_color, false),
        Primitive::StrokeRect {
            rect: Rect {
                x: 25.0,
                y: 8.0,
                width: 14.0,
                height: 14.0,
            },
            color: color(90),
            width: 2.0,
            radius: 2.0,
            static_content: false,
        },
        Primitive::Polyline {
            points: vec![
                cclover_ui::Point { x: 58.0, y: line_y },
                cclover_ui::Point {
                    x: 72.0,
                    y: line_y - 8.0,
                },
                cclover_ui::Point { x: 86.0, y: line_y },
            ],
            color: color(140),
            width: 2.0,
            static_content: false,
        },
        Primitive::Polygon {
            points: vec![
                cclover_ui::Point { x: 70.0, y: 56.0 },
                cclover_ui::Point { x: 80.0, y: 48.0 },
                cclover_ui::Point { x: 88.0, y: 58.0 },
            ],
            color: color(180),
        },
    ])
}

fn must_fit_text(x: f32, width: f32, value: &str) -> Primitive {
    Primitive::Text {
        rect: Rect {
            x,
            y: 40.0,
            width,
            height: 20.0,
        },
        value: value.to_owned(),
        size: 12,
        color: color(255),
        bold: false,
        align: TextAlign::End,
        clip: false,
        must_fit: true,
        static_content: false,
    }
}

struct CairoImage {
    pixels: Vec<u8>,
    surface: *mut c_void,
    cr: *mut c_void,
    width: i32,
    height: i32,
    stride: i32,
}

impl CairoImage {
    fn new(width: i32, height: i32) -> Self {
        let stride = width * 4;
        let mut pixels = vec![0; (stride * height) as usize];
        let surface = unsafe {
            cairo_image_surface_create_for_data(
                pixels.as_mut_ptr(),
                CAIRO_FORMAT_ARGB32,
                width,
                height,
                stride,
            )
        };
        assert!(!surface.is_null(), "Cairo test surface must be available");
        let cr = unsafe { cairo_create(surface) };
        assert!(!cr.is_null(), "Cairo test context must be available");
        unsafe { cclover_cairo_configure_context(cr) };
        Self {
            pixels,
            surface,
            cr,
            width,
            height,
            stride,
        }
    }

    fn flush(&mut self) {
        unsafe { cairo_surface_flush(self.surface) };
    }

    fn pixels(&mut self) -> &[u8] {
        self.flush();
        &self.pixels
    }
}

impl Drop for CairoImage {
    fn drop(&mut self) {
        unsafe {
            cairo_destroy(self.cr);
            cairo_surface_destroy(self.surface);
        }
    }
}

fn cairo_render(
    image: &mut CairoImage,
    frame: &FrameStorage,
    mode: i32,
    cull_to_redraw_mask: bool,
    clear: bool,
) {
    assert_shape_only(frame);
    let view = frame.view();
    assert_eq!(
        unsafe {
            cclover_cairo_validate_scene(image.cr, &view, mode, i32::from(cull_to_redraw_mask))
        },
        1,
        "shape-only test scene must satisfy production Cairo validation"
    );
    unsafe {
        cclover_cairo_execute_validated_scene(
            std::ptr::null_mut(),
            image.cr,
            &view,
            i32::from(clear),
            mode,
            i32::from(cull_to_redraw_mask),
        );
    }
    image.flush();
}

fn cairo_restore_damage(
    image: &mut CairoImage,
    static_layer: &mut CairoImage,
    damage_rects: &[NativeDamageRect],
) {
    image.flush();
    static_layer.flush();
    for rect in damage_rects {
        let x1 = (rect.x1.floor() as i32).clamp(0, image.width);
        let y1 = (rect.y1.floor() as i32).clamp(0, image.height);
        let x2 = (rect.x2.ceil() as i32).clamp(0, image.width);
        let y2 = (rect.y2.ceil() as i32).clamp(0, image.height);
        if x2 <= x1 || y2 <= y1 {
            continue;
        }
        for y in y1..y2 {
            let start = (y * image.stride + x1 * 4) as usize;
            let end = (y * image.stride + x2 * 4) as usize;
            image.pixels[start..end].copy_from_slice(&static_layer.pixels[start..end]);
        }
        unsafe {
            cairo_surface_mark_dirty_rectangle(image.surface, x1, y1, x2 - x1, y2 - y1);
        }
    }
}

fn cairo_render_incremental(image: &mut CairoImage, frame: &FrameStorage) {
    assert_shape_only(frame);
    let view = frame.view();
    assert_eq!(
        unsafe { cclover_cairo_validate_scene(image.cr, &view, CAIRO_DRAW_DYNAMIC, 1) },
        1,
        "candidate dynamic content must validate before incremental execution"
    );
    unsafe {
        cairo_save(image.cr);
        for rect in &frame.damage_rects {
            cairo_rectangle(
                image.cr,
                f64::from(rect.x1),
                f64::from(rect.y1),
                f64::from(rect.x2 - rect.x1),
                f64::from(rect.y2 - rect.y1),
            );
        }
        cairo_clip(image.cr);
        cclover_cairo_execute_validated_scene(
            std::ptr::null_mut(),
            image.cr,
            &view,
            0,
            CAIRO_DRAW_DYNAMIC,
            1,
        );
        cairo_restore(image.cr);
    }
    image.flush();
}

fn assert_shape_only(frame: &FrameStorage) {
    assert!(
        frame
            .commands
            .iter()
            .all(|command| command.kind != COMMAND_TEXT),
        "shape equivalence test intentionally keeps the renderer state pointer unused"
    );
}
