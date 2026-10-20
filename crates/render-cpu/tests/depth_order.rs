use glam::{Vec3, Vec4};
use render_core::{Camera, Projection, Transform};
use render_cpu::{
    render_mesh, Color, Framebuffer, RasterConfig, RasterMesh, RasterVertex, Viewport, Winding,
};

const W: u32 = 64;
const H: u32 = 64;

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

fn quad(z: f32, color: Vec4) -> RasterMesh {
    let v = |x: f32, y: f32| RasterVertex::new(Vec3::new(x, y, z), color);
    RasterMesh::new(
        vec![v(-0.6, -0.6), v(0.6, -0.6), v(0.6, 0.6), v(-0.6, 0.6)],
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn draw(f: &mut Framebuffer, mesh: &RasterMesh, cfg: RasterConfig) -> render_cpu::RasterStats {
    let viewport = Viewport::from_size(W, H).expect("valid viewport");
    render_mesh(f, viewport, cfg, mesh, &camera(), Transform::IDENTITY).expect("renders")
}

fn fb() -> Framebuffer {
    let mut f = Framebuffer::new(W, H).expect("valid framebuffer");
    f.clear(Color::BLACK);
    f
}

#[test]
fn nearer_quad_overwrites_farther_quad() {
    let mut f = fb();
    let far = quad(-1.0, Vec4::new(1.0, 0.0, 0.0, 1.0));
    let near = quad(1.0, Vec4::new(0.0, 0.0, 1.0, 1.0));
    let cfg = RasterConfig::default();
    draw(&mut f, &far, cfg);
    draw(&mut f, &near, cfg);
    let center = f.pixel(W / 2, H / 2).expect("center pixel");
    assert!(center.b > 200 && center.r < 60);
}

#[test]
fn farther_quad_does_not_overwrite_nearer_quad() {
    let mut f = fb();
    let near = quad(1.0, Vec4::new(0.0, 0.0, 1.0, 1.0));
    let far = quad(-1.0, Vec4::new(1.0, 0.0, 0.0, 1.0));
    let cfg = RasterConfig::default();
    draw(&mut f, &near, cfg);
    let stats = draw(&mut f, &far, cfg);
    assert!(stats.pixels_depth_rejected > 0);
    let center = f.pixel(W / 2, H / 2).expect("center pixel");
    assert!(center.b > 200 && center.r < 60);
}

#[test]
fn depth_buffer_records_closer_value() {
    let mut f = fb();
    draw(&mut f, &quad(0.0, Vec4::ONE), RasterConfig::default());
    let depth = f.depth_at(W / 2, H / 2).expect("depth sample");
    assert!(depth < 1.0);
    assert!(depth >= 0.0);
}

#[test]
fn depth_test_disabled_lets_farther_win() {
    let mut f = fb();
    let cfg = RasterConfig {
        winding: Winding::Both,
        depth_test: false,
        depth_write: true,
    };
    draw(&mut f, &quad(1.0, Vec4::new(0.0, 0.0, 1.0, 1.0)), cfg);
    draw(&mut f, &quad(-1.0, Vec4::new(1.0, 0.0, 0.0, 1.0)), cfg);
    let center = f.pixel(W / 2, H / 2).expect("center pixel");
    assert!(center.r > 200 && center.b < 60);
}

#[test]
fn depth_write_disabled_keeps_initial_depth() {
    let mut f = fb();
    let cfg = RasterConfig {
        winding: Winding::Both,
        depth_test: true,
        depth_write: false,
    };
    draw(&mut f, &quad(0.0, Vec4::ONE), cfg);
    assert_eq!(f.depth_at(W / 2, H / 2), Some(1.0));
}
