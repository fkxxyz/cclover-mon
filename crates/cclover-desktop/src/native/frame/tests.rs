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
