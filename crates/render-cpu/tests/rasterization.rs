use glam::{Vec3, Vec4};
use render_core::{Camera, Projection, Transform};
use render_cpu::{
    render_mesh, Color, Framebuffer, RasterConfig, RasterMesh, RasterStats, RasterVertex, Viewport,
    Winding,
};

const W: u32 = 64;
const H: u32 = 64;

fn fb() -> Framebuffer {
    let mut f = Framebuffer::new(W, H).expect("valid framebuffer");
    f.clear(Color::BLACK);
    f
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

fn tri(a: Vec3, b: Vec3, c: Vec3, color: Vec4) -> RasterMesh {
    RasterMesh::triangle(
        RasterVertex::new(a, color),
        RasterVertex::new(b, color),
        RasterVertex::new(c, color),
    )
}

fn count_non_black(f: &Framebuffer) -> usize {
    f.color_bytes()
        .chunks_exact(4)
        .filter(|c| *c != [0, 0, 0, 255])
        .count()
}

fn draw(f: &mut Framebuffer, mesh: &RasterMesh, cfg: RasterConfig) -> RasterStats {
    let viewport = Viewport::from_size(W, H).expect("valid viewport");
    render_mesh(f, viewport, cfg, mesh, &camera(), Transform::IDENTITY).expect("renders")
}

#[test]
fn triangle_covers_expected_pixels() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(-0.5, -0.5, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec3::new(0.0, 0.5, 0.0),
        Vec4::new(1.0, 1.0, 1.0, 1.0),
    );
    let stats = draw(&mut f, &mesh, RasterConfig::default());
    assert!(stats.pixels_written > 0);
    assert_eq!(count_non_black(&f), stats.pixels_written as usize);
    assert_eq!(stats.triangles_drawn, 1);
}

#[test]
fn triangle_outside_viewport_writes_nothing() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(10.0, 10.0, 0.0),
        Vec3::new(11.0, 10.0, 0.0),
        Vec3::new(10.0, 11.0, 0.0),
        Vec4::ONE,
    );
    let stats = draw(&mut f, &mesh, RasterConfig::default());
    assert_eq!(stats.pixels_written, 0);
    assert_eq!(count_non_black(&f), 0);
    assert_eq!(stats.triangles_offscreen, 1);
}

#[test]
fn degenerate_triangle_is_rejected() {
    let mut f = fb();
    let mesh = tri(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec4::ONE);
    let stats = draw(&mut f, &mesh, RasterConfig::default());
    assert_eq!(stats.pixels_written, 0);
    assert_eq!(stats.triangles_degenerate, 1);
}

#[test]
fn collinear_triangle_is_rejected() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(-0.5, -0.5, 0.0),
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec4::ONE,
    );
    let stats = draw(&mut f, &mesh, RasterConfig::default());
    assert_eq!(stats.pixels_written, 0);
    assert_eq!(stats.triangles_degenerate, 1);
}

#[test]
fn both_winding_orders_are_accepted() {
    let mut f1 = fb();
    let mut f2 = fb();
    let ccw = tri(
        Vec3::new(-0.5, -0.5, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec3::new(0.0, 0.5, 0.0),
        Vec4::ONE,
    );
    let cw = tri(
        Vec3::new(-0.5, -0.5, 0.0),
        Vec3::new(0.0, 0.5, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec4::ONE,
    );
    let cfg = RasterConfig::default();
    draw(&mut f1, &ccw, cfg);
    draw(&mut f2, &cw, cfg);
    assert!(count_non_black(&f1) > 0);
    assert_eq!(count_non_black(&f1), count_non_black(&f2));
}

#[test]
fn winding_culling_discards_back_faces() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(-0.5, -0.5, 0.0),
        Vec3::new(0.0, 0.5, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec4::ONE,
    );
    let stats = draw(
        &mut f,
        &mesh,
        RasterConfig {
            winding: Winding::Front,
            depth_test: true,
            depth_write: true,
        },
    );
    assert_eq!(stats.pixels_written, 0);
    assert_eq!(stats.triangles_culled, 1);
}

#[test]
fn colors_are_interpolated_across_triangle() {
    let mut f = fb();
    let mesh = RasterMesh::triangle(
        RasterVertex::new(Vec3::new(-0.5, -0.5, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
        RasterVertex::new(Vec3::new(0.5, -0.5, 0.0), Vec4::new(0.0, 1.0, 0.0, 1.0)),
        RasterVertex::new(Vec3::new(0.0, 0.5, 0.0), Vec4::new(0.0, 0.0, 1.0, 1.0)),
    );
    draw(&mut f, &mesh, RasterConfig::default());
    let (mut r, mut g, mut b) = (false, false, false);
    for c in f.color_bytes().chunks_exact(4) {
        if c[0] > 200 && c[1] < 60 && c[2] < 60 {
            r = true;
        }
        if c[1] > 200 && c[0] < 60 && c[2] < 60 {
            g = true;
        }
        if c[2] > 200 && c[0] < 60 && c[1] < 60 {
            b = true;
        }
    }
    assert!(r && g && b);
}

#[test]
fn non_finite_vertex_is_rejected() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(f32::NAN, 0.0, 0.0),
        Vec3::new(0.5, -0.5, 0.0),
        Vec3::new(0.0, 0.5, 0.0),
        Vec4::ONE,
    );
    let viewport = Viewport::from_size(W, H).expect("valid viewport");
    let result = render_mesh(
        &mut f,
        viewport,
        RasterConfig::default(),
        &mesh,
        &camera(),
        Transform::IDENTITY,
    );
    assert!(result.is_err());
}

#[test]
fn vertices_behind_camera_are_clipped() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(-0.5, -0.5, 5.0),
        Vec3::new(0.5, -0.5, 5.0),
        Vec3::new(0.0, 0.5, 5.0),
        Vec4::ONE,
    );
    let stats = draw(&mut f, &mesh, RasterConfig::default());
    assert_eq!(stats.pixels_written, 0);
    assert_eq!(stats.triangles_clipped, 1);
}

#[test]
fn triangle_crossing_near_plane_is_clipped_not_dropped() {
    let mut f = fb();
    let mesh = tri(
        Vec3::new(-0.5, -0.5, 6.0),
        Vec3::new(0.5, -0.5, 1.0),
        Vec3::new(0.0, 0.5, 1.0),
        Vec4::ONE,
    );
    let stats = draw(&mut f, &mesh, RasterConfig::default());
    assert_eq!(stats.triangles_clipped, 1);
    assert!(stats.pixels_written > 0);
}
