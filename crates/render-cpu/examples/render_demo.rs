use std::path::PathBuf;

use glam::{Vec3, Vec4};
use render_core::{Camera, Projection, Transform};
use render_cpu::{
    render_mesh, write_png, Color, Framebuffer, RasterConfig, RasterMesh, RasterVertex, Viewport,
    Winding,
};

fn output_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.push("target");
    dir.push("render-output");
    dir
}

fn demo_camera(aspect: f32) -> Camera {
    Camera::new(
        Vec3::new(2.6, 2.2, 4.2),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 50.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 100.0,
        },
    )
    .expect("valid camera")
}

fn render_triangle(width: u32, height: u32) -> Framebuffer {
    let mut fb = Framebuffer::new(width, height).expect("valid framebuffer");
    fb.clear(Color::new(12, 16, 28, 255));
    let viewport = Viewport::from_size(width, height).expect("valid viewport");
    let camera = demo_camera(width as f32 / height as f32);
    let mesh = RasterMesh::triangle(
        RasterVertex::new(Vec3::new(-1.0, -0.8, 0.0), Vec4::new(1.0, 0.25, 0.2, 1.0)),
        RasterVertex::new(Vec3::new(1.0, -0.8, 0.0), Vec4::new(0.2, 1.0, 0.35, 1.0)),
        RasterVertex::new(Vec3::new(0.0, 1.0, 0.0), Vec4::new(0.3, 0.45, 1.0, 1.0)),
    );
    let config = RasterConfig {
        winding: Winding::Both,
        depth_test: true,
        depth_write: true,
    };
    let stats = render_mesh(
        &mut fb,
        viewport,
        config,
        &mesh,
        &camera,
        Transform::IDENTITY,
    )
    .expect("triangle renders");
    println!(
        "triangle: triangles={} drawn={} pixels={} degenerate={}",
        stats.triangles, stats.triangles_drawn, stats.pixels_written, stats.triangles_degenerate
    );
    fb
}

fn render_cube(width: u32, height: u32) -> Framebuffer {
    let mut fb = Framebuffer::new(width, height).expect("valid framebuffer");
    fb.clear(Color::new(10, 12, 20, 255));
    let viewport = Viewport::from_size(width, height).expect("valid viewport");
    let camera = demo_camera(width as f32 / height as f32);
    let mesh = RasterMesh::cube(Vec3::ZERO, 1.8, Vec4::new(0.35, 0.72, 1.0, 1.0));
    let config = RasterConfig {
        winding: Winding::Both,
        depth_test: true,
        depth_write: true,
    };
    let transform = Transform::new(
        Vec3::new(0.0, 0.35, 0.0),
        glam::Quat::from_rotation_y(0.7),
        Vec3::ONE,
    );
    let stats =
        render_mesh(&mut fb, viewport, config, &mesh, &camera, transform).expect("cube renders");
    println!(
        "cube: triangles={} drawn={} pixels={} depth_rejected={}",
        stats.triangles, stats.triangles_drawn, stats.pixels_written, stats.pixels_depth_rejected
    );
    fb
}

fn non_background_pixels(fb: &Framebuffer) -> usize {
    let bytes = fb.color_bytes();
    let mut count = 0usize;
    for chunk in bytes.chunks_exact(4) {
        if chunk != [10, 12, 20, 255] && chunk != [12, 16, 28, 255] {
            count += 1;
        }
    }
    count
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir)?;

    let width = 640u32;
    let height = 480u32;

    let triangle = render_triangle(width, height);
    let triangle_path = dir.join("triangle.png");
    write_png(&triangle_path, &triangle)?;
    let tri_pixels = non_background_pixels(&triangle);
    println!(
        "wrote {} ({} non background pixels)",
        triangle_path.display(),
        tri_pixels
    );
    if tri_pixels == 0 {
        return Err("triangle.png is empty".into());
    }

    let cube = render_cube(width, height);
    let cube_path = dir.join("cube.png");
    write_png(&cube_path, &cube)?;
    let cube_pixels = non_background_pixels(&cube);
    println!(
        "wrote {} ({} non background pixels)",
        cube_path.display(),
        cube_pixels
    );
    if cube_pixels == 0 {
        return Err("cube.png is empty".into());
    }

    Ok(())
}
