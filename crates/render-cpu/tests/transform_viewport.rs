use glam::{Mat4, Vec3, Vec4};
use render_core::{Camera, Projection, Transform};
use render_cpu::{model_matrix, mvp, transform_vertex, Viewport};

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn camera() -> Camera {
    Camera::new(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 1.0,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    )
    .expect("valid camera")
}

#[test]
fn model_matrix_applies_transform() {
    let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let m = model_matrix(t);
    assert_eq!(m.transform_point3(Vec3::ZERO), Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn mvp_places_origin_at_center_of_screen() {
    let cam = camera();
    let m = mvp(&cam, Transform::IDENTITY);
    let clip = m * Vec4::new(0.0, 0.0, 0.0, 1.0);
    assert!(approx(clip.x / clip.w, 0.0));
    assert!(approx(clip.y / clip.w, 0.0));
}

#[test]
fn mvp_preserves_relative_scale() {
    let cam = camera();
    let a = mvp(&cam, Transform::IDENTITY) * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let b = mvp(&cam, Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)))
        * Vec4::new(0.0, 0.0, 0.0, 1.0);
    assert!(approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z));
}

#[test]
fn mvp_translation_changes_ndc() {
    let cam = camera();
    let base = mvp(&cam, Transform::IDENTITY) * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let moved = mvp(&cam, Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)))
        * Vec4::new(0.0, 0.0, 0.0, 1.0);
    assert!(moved.x / moved.w > base.x / base.w);
}

#[test]
fn mvp_depth_matches_distance_to_camera() {
    let cam = camera();
    let near = mvp(&cam, Transform::IDENTITY) * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let far = mvp(&cam, Transform::from_translation(Vec3::new(0.0, 0.0, -2.0)))
        * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let near_z = near.z / near.w;
    let far_z = far.z / far.w;
    assert!(near_z >= 0.0 && near_z <= 1.0);
    assert!(far_z > near_z);
}

#[test]
fn transform_vertex_matches_matrix_product() {
    let cam = camera();
    let m = mvp(&cam, Transform::IDENTITY);
    let v = transform_vertex(&m, Vec3::new(0.25, -0.5, 0.0), Vec4::ONE);
    let expected = m * Vec4::new(0.25, -0.5, 0.0, 1.0);
    assert!(approx(v.clip.x, expected.x));
    assert!(approx(v.clip.y, expected.y));
    assert!(approx(v.clip.z, expected.z));
    assert!(approx(v.clip.w, expected.w));
}

#[test]
fn identity_transform_is_identity_matrix() {
    assert_eq!(model_matrix(Transform::IDENTITY), Mat4::IDENTITY);
}

#[test]
fn viewport_maps_ndc_center_to_pixel_center() {
    let vp = Viewport::from_size(640, 480).expect("valid viewport");
    let (x, y) = vp.ndc_to_pixel(0.0, 0.0);
    assert!(approx(x, 320.0));
    assert!(approx(y, 240.0));
}

#[test]
fn viewport_maps_ndc_corners_to_pixel_corners() {
    let vp = Viewport::from_size(640, 480).expect("valid viewport");
    let (x0, y0) = vp.ndc_to_pixel(-1.0, 1.0);
    let (x1, y1) = vp.ndc_to_pixel(1.0, -1.0);
    assert!(approx(x0, 0.0));
    assert!(approx(y0, 0.0));
    assert!(approx(x1, 640.0));
    assert!(approx(y1, 480.0));
}

#[test]
fn viewport_pixel_to_ndc_is_inverse() {
    let vp = Viewport::from_size(320, 240).expect("valid viewport");
    let (nx, ny) = vp.pixel_to_ndc(160.0, 120.0);
    assert!(approx(nx, 0.0));
    assert!(approx(ny, 0.0));
    let (px, py) = vp.ndc_to_pixel(nx, ny);
    assert!(approx(px, 160.0));
    assert!(approx(py, 120.0));
}

#[test]
fn viewport_y_axis_is_flipped_for_screen_space() {
    let vp = Viewport::from_size(100, 100).expect("valid viewport");
    let (_, top) = vp.ndc_to_pixel(0.0, 1.0);
    let (_, bottom) = vp.ndc_to_pixel(0.0, -1.0);
    assert!(top < bottom);
}

#[test]
fn viewport_with_offset_is_applied() {
    let vp = Viewport::new(10.0, 20.0, 100.0, 50.0).expect("valid viewport");
    let (x, y) = vp.ndc_to_pixel(0.0, 0.0);
    assert!(approx(x, 60.0));
    assert!(approx(y, 45.0));
}

#[test]
fn viewport_rejects_invalid_dimensions() {
    assert!(Viewport::new(0.0, 0.0, 0.0, 100.0).is_err());
    assert!(Viewport::new(0.0, 0.0, 100.0, -1.0).is_err());
    assert!(Viewport::new(f32::NAN, 0.0, 100.0, 100.0).is_err());
    assert!(Viewport::new(f32::INFINITY, 0.0, 100.0, 100.0).is_err());
}
