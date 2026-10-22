use glam::Vec4;
use render_cpu::{clip_polygon_near, clip_triangle_near, ClipVertex};

fn v(x: f32, y: f32, z: f32, w: f32) -> ClipVertex {
    ClipVertex::new(Vec4::new(x, y, z, w), Vec4::ONE)
}

#[test]
fn polygon_fully_in_front_is_unchanged() {
    let poly = vec![
        v(0.0, 0.0, 0.0, 1.0),
        v(1.0, 0.0, 0.0, 1.0),
        v(0.0, 1.0, 0.0, 1.0),
    ];
    let out = clip_polygon_near(&poly).expect("clips");
    assert_eq!(out.len(), 3);
}

#[test]
fn polygon_fully_behind_is_emptied() {
    let poly = vec![
        v(0.0, 0.0, 0.0, -1.0),
        v(1.0, 0.0, 0.0, -1.0),
        v(0.0, 1.0, 0.0, -1.0),
    ];
    let out = clip_polygon_near(&poly).expect("clips");
    assert!(out.is_empty());
}

#[test]
fn polygon_crossing_near_plane_grows_to_quad() {
    let poly = vec![
        v(0.0, 0.0, 0.0, -1.0),
        v(1.0, 0.0, 0.0, 1.0),
        v(0.0, 1.0, 0.0, 1.0),
    ];
    let out = clip_polygon_near(&poly).expect("clips");
    assert_eq!(out.len(), 4);
    assert!(out.iter().all(|c| c.clip.w > 0.0));
}

#[test]
fn clipped_vertex_interpolates_color() {
    let a = ClipVertex::new(
        Vec4::new(0.0, 0.0, 0.0, -1.0),
        Vec4::new(1.0, 0.0, 0.0, 1.0),
    );
    let b = ClipVertex::new(Vec4::new(0.0, 0.0, 0.0, 1.0), Vec4::new(0.0, 1.0, 0.0, 1.0));
    let c = ClipVertex::new(Vec4::new(0.0, 1.0, 0.0, 1.0), Vec4::new(0.0, 0.0, 1.0, 1.0));
    let out = clip_triangle_near(a, b, c).expect("clips");
    assert_eq!(out.len(), 4);
    for vert in &out {
        assert!((vert.color.x + vert.color.y + vert.color.z - 1.0).abs() < 1e-4);
    }
}

#[test]
fn non_finite_polygon_is_rejected() {
    let poly = vec![
        v(f32::NAN, 0.0, 0.0, 1.0),
        v(1.0, 0.0, 0.0, 1.0),
        v(0.0, 1.0, 0.0, 1.0),
    ];
    assert!(clip_polygon_near(&poly).is_err());
}

#[test]
fn degenerate_polygon_returns_empty() {
    let poly = vec![v(0.0, 0.0, 0.0, 1.0)];
    let out = clip_polygon_near(&poly).expect("clips");
    assert!(out.is_empty());
}
