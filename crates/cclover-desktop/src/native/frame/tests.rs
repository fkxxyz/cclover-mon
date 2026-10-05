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

fn scene(primitives: Vec<Primitive>) -> Scene {
    Scene {
        width: 100,
        height: 100,
        primitives,
    }
}

#[test]
fn must_fit_text_crosses_native_abi_as_an_explicit_flag() {
    let primitive = Primitive::Text {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 42.0,
            height: 16.0,
        },
        value: "42.0%".to_owned(),
        size: 12,
        color: color(255),
        bold: true,
        align: TextAlign::End,
        clip: false,
        must_fit: true,
        static_content: false,
    };
    let mut commands = Vec::new();
    let mut points = Vec::new();

    push_command(&mut commands, &mut points, &primitive);

    assert_eq!(commands.len(), 1);
    assert_ne!(commands[0].flags & FLAG_TEXT_MUST_FIT, 0);
    assert_ne!(commands[0].flags & FLAG_TEXT_BOLD, 0);
    assert_ne!(commands[0].flags & FLAG_TEXT_END, 0);
    assert_eq!(commands[0].flags & FLAG_TEXT_CLIP, 0);
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
        (
            damage.x,
            damage.y,
            damage.x + damage.width,
            damage.y + damage.height,
        ),
        (9.0, 9.0, 31.0, 21.0)
    );
}

#[test]
fn incremental_redraw_mask_includes_changed_and_overlapping_dynamic_commands() {
    let previous = scene(vec![
        fill(0.0, color(1), true),
        fill(10.0, color(2), false),
        fill(32.0, color(3), false),
        fill(70.0, color(4), false),
    ]);
    let current = scene(vec![
        fill(0.0, color(1), true),
        fill(20.0, color(2), false),
        fill(32.0, color(3), false),
        fill(70.0, color(4), false),
    ]);

    let frame = FrameStorage::from_scene(current, Some(&previous), 7);

    assert!(!frame.full_redraw);
    assert_eq!(frame.commands.len(), 4);
    assert_eq!(frame.redraw_mask, vec![0, 1, 1, 0]);
    let view = frame.view();
    assert_eq!(view.redraw_mask_count, view.command_count);
    assert!(!view.redraw_mask.is_null());
}

#[test]
fn full_redraw_omits_incremental_redraw_mask() {
    let current = scene(vec![fill(0.0, color(1), true), fill(20.0, color(2), false)]);

    let frame = FrameStorage::from_scene(current, None, 0);

    assert!(frame.full_redraw);
    assert!(frame.redraw_mask.is_empty());
    let view = frame.view();
    assert_eq!(view.redraw_mask_count, 0);
    assert!(view.redraw_mask.is_null());
}

#[test]
fn unchanged_incremental_scene_keeps_a_complete_zero_mask() {
    let previous = scene(vec![fill(0.0, color(1), true), fill(20.0, color(2), false)]);
    let current = previous.clone();

    let frame = FrameStorage::from_scene(current, Some(&previous), 7);

    assert!(!frame.full_redraw);
    assert_eq!(frame.redraw_mask, vec![0, 0]);
    assert_eq!(frame.redraw_mask.len(), frame.commands.len());
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
fn appended_dynamic_primitive_damages_only_its_bounds() {
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

    assert!(!invalidation.full_redraw);
    assert_eq!(invalidation.static_revision, 7);
    assert_eq!(invalidation.damage_rects.len(), 1);
    let damage = invalidation.damage_rects[0];
    assert_eq!(
        (
            damage.x,
            damage.y,
            damage.x + damage.width,
            damage.y + damage.height,
        ),
        (39.0, 9.0, 51.0, 21.0)
    );
}

#[test]
fn removed_dynamic_primitive_damages_its_old_bounds() {
    let previous = scene(vec![
        fill(10.0, color(1), true),
        fill(20.0, color(2), false),
        fill(40.0, color(3), false),
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
        (
            damage.x,
            damage.y,
            damage.x + damage.width,
            damage.y + damage.height,
        ),
        (39.0, 9.0, 51.0, 21.0)
    );
}

#[test]
fn inserted_dynamic_primitive_conservatively_damages_shifted_tail() {
    let previous = scene(vec![
        fill(10.0, color(1), true),
        fill(20.0, color(2), false),
        fill(60.0, color(3), false),
    ]);
    let current = scene(vec![
        fill(10.0, color(1), true),
        fill(20.0, color(2), false),
        fill(40.0, color(4), false),
        fill(60.0, color(3), false),
    ]);

    let invalidation = scene_invalidation(Some(&previous), &current, 7);

    assert!(!invalidation.full_redraw);
    assert_eq!(invalidation.static_revision, 7);
    assert_eq!(invalidation.damage_rects.len(), 1);
    let damage = invalidation.damage_rects[0];
    assert_eq!(
        (
            damage.x,
            damage.y,
            damage.x + damage.width,
            damage.y + damage.height,
        ),
        (39.0, 9.0, 71.0, 21.0)
    );
}
