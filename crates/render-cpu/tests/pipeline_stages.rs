use glam::{Mat4, Vec3, Vec4};
use render_cpu::pipeline::raster::{raster_triangle, to_screen, FillRule, Fragment, ScreenVertex};
use render_cpu::pipeline::CpuPipeline;
use render_cpu::{
    Color, Framebuffer, PipelineConfig, RasterMesh, RasterVertex, Topology, Viewport,
};

fn vertex(x: f32, y: f32, z: f32) -> RasterVertex {
    RasterVertex::new(Vec3::new(x, y, z), Vec4::new(1.0, 1.0, 1.0, 1.0))
}

fn framebuffer() -> Framebuffer {
    Framebuffer::new(16, 16).expect("framebuffer")
}

fn screen(x: f32, y: f32) -> ScreenVertex {
    ScreenVertex {
        x,
        y,
        z: 0.5,
        inv_w: 1.0,
        color: Vec4::ONE,
    }
}

#[test]
fn pipeline_draws_a_triangle() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.5),
        vertex(1.0, -1.0, 0.5),
        vertex(0.0, 1.0, 0.5),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    let stats = pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    assert_eq!(stats.metrics.triangles_assembled, 1);
    assert!(stats.metrics.pixels_written > 0);
    assert_eq!(stats.metrics.fragments_depth_rejected, 0);
}

#[test]
fn pipeline_reports_stage_counters() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.5),
        vertex(1.0, -1.0, 0.5),
        vertex(0.0, 1.0, 0.5),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    let stats = pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    assert_eq!(stats.metrics.vertices_in, 3);
    assert_eq!(stats.metrics.vertices_out, 3);
    assert_eq!(stats.metrics.triangles_assembled, 1);
    assert_eq!(stats.metrics.triangles_rejected, 0);
    assert_eq!(stats.draws, 1);
    assert_eq!(stats.errors, 0);
}

#[test]
fn pipeline_culls_triangles_outside_the_frustum() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, -5.0),
        vertex(1.0, -1.0, -5.0),
        vertex(0.0, 1.0, -5.0),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    let stats = pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    assert_eq!(stats.metrics.triangles_culled, 1);
    assert_eq!(stats.metrics.pixels_written, 0);
}

#[test]
fn pipeline_clips_triangles_crossing_the_frustum() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, -1.0),
        vertex(1.0, -1.0, 0.5),
        vertex(0.0, 1.0, 0.5),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    let stats = pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    assert_eq!(stats.metrics.triangles_clipped, 1);
    assert!(stats.metrics.pixels_written > 0);
}

#[test]
fn depth_test_rejects_farther_fragments() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let near = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.2),
        vertex(1.0, -1.0, 0.2),
        vertex(0.0, 1.0, 0.2),
    );
    let far = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.8),
        vertex(1.0, -1.0, 0.8),
        vertex(0.0, 1.0, 0.8),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    let first = pipeline
        .draw_mesh(&near, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    let stats = pipeline
        .draw_mesh(&far, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    let written = stats.metrics.pixels_written - first.metrics.pixels_written;
    let rejected = stats.metrics.fragments_depth_rejected - first.metrics.fragments_depth_rejected;
    assert!(rejected > 0);
    assert_eq!(written, 0);
}

#[test]
fn depth_test_disabled_lets_far_fragments_overwrite() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let config = PipelineConfig {
        depth_test: false,
        depth_write: false,
        ..PipelineConfig::default()
    };
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.8),
        vertex(1.0, -1.0, 0.8),
        vertex(0.0, 1.0, 0.8),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, config);
    let stats = pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    assert_eq!(stats.metrics.fragments_depth_rejected, 0);
    assert!(stats.metrics.pixels_written > 0);
    assert_eq!(fb.depth_at(8, 8), Some(1.0));
}

#[test]
fn top_left_rule_does_not_double_fill_shared_edges() {
    let a = screen(2.0, 2.0);
    let b = screen(10.0, 2.0);
    let c = screen(2.0, 10.0);
    let mut fragments = Vec::new();
    raster_triangle(&a, &b, &c, (16, 16), FillRule::TopLeft, &mut fragments);
    let top_left = fragments.len();
    fragments.clear();
    raster_triangle(&a, &b, &c, (16, 16), FillRule::All, &mut fragments);
    let all = fragments.len();
    assert!(top_left <= all);
    assert!(top_left > 0);
}

#[test]
fn raster_triangle_outside_framebuffer_produces_no_fragments() {
    let a = screen(-100.0, -100.0);
    let b = screen(-50.0, -100.0);
    let c = screen(-100.0, -50.0);
    let mut fragments = Vec::new();
    let count = raster_triangle(&a, &b, &c, (16, 16), FillRule::TopLeft, &mut fragments);
    assert_eq!(count, 0);
    assert!(fragments.is_empty());
}

#[test]
fn degenerate_screen_triangle_produces_no_fragments() {
    let a = screen(4.0, 4.0);
    let b = screen(4.0, 4.0);
    let c = screen(4.0, 4.0);
    let mut fragments = Vec::new();
    let count = raster_triangle(&a, &b, &c, (16, 16), FillRule::TopLeft, &mut fragments);
    assert_eq!(count, 0);
}

#[test]
fn to_screen_maps_ndc_to_pixels() {
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let clip = render_cpu::ClipVertex::new(Vec4::new(0.0, 0.0, 0.5, 1.0), Vec4::ONE);
    let s = to_screen(&clip, &viewport).expect("screen vertex");
    assert!((s.x - 8.0).abs() < 1e-4);
    assert!((s.y - 8.0).abs() < 1e-4);
    assert!((s.z - 0.5).abs() < 1e-6);
    assert!((s.inv_w - 1.0).abs() < 1e-6);
}

#[test]
fn to_screen_rejects_zero_w() {
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let clip = render_cpu::ClipVertex::new(Vec4::new(0.0, 0.0, 0.5, 0.0), Vec4::ONE);
    assert!(to_screen(&clip, &viewport).is_none());
}

#[test]
fn pipeline_writes_expected_color() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let red = Vec4::new(1.0, 0.0, 0.0, 1.0);
    let mesh = RasterMesh::triangle(
        RasterVertex::new(Vec3::new(-1.0, -1.0, 0.5), red),
        RasterVertex::new(Vec3::new(1.0, -1.0, 0.5), red),
        RasterVertex::new(Vec3::new(0.0, 1.0, 0.5), red),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    let pixel = fb.pixel(8, 8).expect("pixel");
    assert_eq!(pixel.r, 255);
    assert_eq!(pixel.g, 0);
    assert_eq!(pixel.b, 0);
}

#[test]
fn pipeline_accumulates_stats_across_draws() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.5),
        vertex(1.0, -1.0, 0.5),
        vertex(0.0, 1.0, 0.5),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    let stats = pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    assert_eq!(stats.draws, 2);
    assert_eq!(stats.metrics.triangles_assembled, 2);
    assert_eq!(stats.metrics.vertices_in, 6);
}

#[test]
fn pipeline_rejects_non_finite_vertices() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let bad = RasterVertex::new(Vec3::new(f32::NAN, 0.0, 0.0), Vec4::ONE);
    let mesh = RasterMesh::triangle(bad, bad, bad);
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    assert!(pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .is_err());
}

#[test]
fn pipeline_respects_depth_buffer_values() {
    let mut fb = framebuffer();
    let viewport = Viewport::from_size(16, 16).expect("viewport");
    let mesh = RasterMesh::triangle(
        vertex(-1.0, -1.0, 0.25),
        vertex(1.0, -1.0, 0.25),
        vertex(0.0, 1.0, 0.25),
    );
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    pipeline
        .draw_mesh(&mesh, Mat4::IDENTITY, Topology::TriangleList)
        .expect("draws");
    let depth = fb.depth_at(8, 8).expect("depth");
    assert!((depth - 0.25).abs() < 1e-3, "depth {depth}");
}
