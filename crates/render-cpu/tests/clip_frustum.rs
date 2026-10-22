use glam::Vec4;
use render_cpu::clip::{clip_polygon, clip_triangle, ClipPlane, ClipVertex};
use render_cpu::RasterError;

fn v(x: f32, y: f32, z: f32, w: f32) -> ClipVertex {
    ClipVertex::new(Vec4::new(x, y, z, w), Vec4::ONE)
}

fn inside_all(clip: &Vec4) -> bool {
    ClipPlane::ALL.iter().all(|p| p.distance(clip) >= -1e-5)
}

#[test]
fn fully_visible_triangle_is_unchanged() {
    let a = v(0.0, 0.0, 0.5, 1.0);
    let b = v(0.5, 0.0, 0.5, 1.0);
    let c = v(0.0, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn triangle_fully_outside_near_is_removed() {
    let a = v(0.0, 0.0, -1.0, 1.0);
    let b = v(0.5, 0.0, -1.0, 1.0);
    let c = v(0.0, 0.5, -1.0, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.is_empty());
}

#[test]
fn triangle_crossing_near_is_clipped_to_visible_part() {
    let a = v(0.0, 0.0, -1.0, 1.0);
    let b = v(0.5, 0.0, 0.5, 1.0);
    let c = v(0.0, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert_eq!(out.len(), 4);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
    assert!(out.iter().all(|x| x.clip.z >= -1e-5));
}

#[test]
fn triangle_crossing_far_is_clipped() {
    let a = v(0.0, 0.0, 0.5, 1.0);
    let b = v(0.5, 0.0, 2.0, 1.0);
    let c = v(0.0, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn triangle_crossing_right_is_clipped() {
    let a = v(2.0, 0.0, 0.5, 1.0);
    let b = v(-0.5, 0.0, 0.5, 1.0);
    let c = v(0.5, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn triangle_crossing_top_is_clipped() {
    let a = v(0.0, 2.0, 0.5, 1.0);
    let b = v(0.0, -0.5, 0.5, 1.0);
    let c = v(0.5, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn triangle_crossing_bottom_is_clipped() {
    let a = v(0.0, -2.0, 0.5, 1.0);
    let b = v(0.0, 0.5, 0.5, 1.0);
    let c = v(0.5, -0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn triangle_crossing_multiple_planes_is_clipped() {
    let a = v(-2.0, -2.0, -1.0, 1.0);
    let b = v(2.0, -2.0, 0.5, 1.0);
    let c = v(-2.0, 2.0, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn triangle_crossing_left_is_clipped() {
    let a = v(-2.0, 0.0, 0.5, 1.0);
    let b = v(0.5, 0.0, 0.5, 1.0);
    let c = v(-0.5, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    assert!(out.iter().all(|x| inside_all(&x.clip)));
}

#[test]
fn vertex_exactly_on_plane_is_kept() {
    let a = v(-1.0, 0.0, 0.5, 1.0);
    let b = v(0.5, 0.0, 0.5, 1.0);
    let c = v(0.0, 0.5, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert_eq!(out.len(), 3);
    assert!(out.iter().any(|x| (x.clip.x + x.clip.w).abs() < 1e-6));
}

#[test]
fn triangle_behind_camera_is_removed() {
    let a = v(0.0, 0.0, 0.5, -1.0);
    let b = v(0.5, 0.0, 0.5, -1.0);
    let c = v(0.0, 0.5, 0.5, -1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.is_empty());
}

#[test]
fn non_finite_polygon_is_rejected() {
    let poly = vec![
        v(f32::NAN, 0.0, 0.0, 1.0),
        v(1.0, 0.0, 0.0, 1.0),
        v(0.0, 1.0, 0.0, 1.0),
    ];
    assert_eq!(clip_polygon(&poly).unwrap_err(), RasterError::InvalidClip);
}

#[test]
fn infinite_polygon_is_rejected() {
    let poly = vec![
        v(f32::INFINITY, 0.0, 0.0, 1.0),
        v(1.0, 0.0, 0.0, 1.0),
        v(0.0, 1.0, 0.0, 1.0),
    ];
    assert_eq!(clip_polygon(&poly).unwrap_err(), RasterError::InvalidClip);
}

#[test]
fn interpolated_attributes_stay_continuous() {
    let a = ClipVertex::new(
        Vec4::new(0.0, 0.0, -1.0, 1.0),
        Vec4::new(1.0, 0.0, 0.0, 1.0),
    );
    let b = ClipVertex::new(Vec4::new(0.0, 0.0, 0.5, 1.0), Vec4::new(0.0, 1.0, 0.0, 1.0));
    let c = ClipVertex::new(Vec4::new(0.0, 1.0, 0.5, 1.0), Vec4::new(0.0, 0.0, 1.0, 1.0));
    let out = clip_triangle(a, b, c).expect("clips");
    for vert in &out {
        let sum = vert.color.x + vert.color.y + vert.color.z;
        assert!((sum - 1.0).abs() < 1e-4, "color sum {sum}");
    }
}

#[test]
fn degenerate_triangle_collapses_to_zero_area() {
    let a = v(0.0, 0.0, 0.5, 1.0);
    let out = clip_triangle(a, a, a).expect("clips");
    assert!(polygon_signed_area(&out).abs() < 1e-6);
}

#[test]
fn clipping_preserves_winding_order() {
    let a = v(-2.0, -2.0, -1.0, 1.0);
    let b = v(2.0, -2.0, 0.5, 1.0);
    let c = v(-2.0, 2.0, 0.5, 1.0);
    let out = clip_triangle(a, b, c).expect("clips");
    assert!(out.len() >= 3);
    let area = polygon_signed_area(&out);
    assert!(area.abs() > 1e-6, "polygon must keep non zero area");
}

fn polygon_signed_area(poly: &[ClipVertex]) -> f32 {
    let mut area = 0.0;
    for i in 0..poly.len() {
        let a = poly[i].clip;
        let b = poly[(i + 1) % poly.len()].clip;
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
}

#[test]
fn plane_distances_follow_wgpu_convention() {
    let inside = v(0.0, 0.0, 0.5, 1.0);
    for plane in ClipPlane::ALL {
        assert!(plane.distance(&inside.clip) > 0.0);
    }
    assert!(ClipPlane::Left.distance(&v(-2.0, 0.0, 0.5, 1.0).clip) < 0.0);
    assert!(ClipPlane::Right.distance(&v(2.0, 0.0, 0.5, 1.0).clip) < 0.0);
    assert!(ClipPlane::Bottom.distance(&v(0.0, -2.0, 0.5, 1.0).clip) < 0.0);
    assert!(ClipPlane::Top.distance(&v(0.0, 2.0, 0.5, 1.0).clip) < 0.0);
    assert!(ClipPlane::Near.distance(&v(0.0, 0.0, -1.0, 1.0).clip) < 0.0);
    assert!(ClipPlane::Far.distance(&v(0.0, 0.0, 2.0, 1.0).clip) < 0.0);
}

#[test]
fn polygon_with_fewer_than_three_vertices_is_empty() {
    let poly = vec![v(0.0, 0.0, 0.5, 1.0)];
    assert!(clip_polygon(&poly).expect("clips").is_empty());
}
