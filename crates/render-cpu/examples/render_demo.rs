use std::path::PathBuf;

use render_core::Transform;
use render_cpu::{
    demo_camera, demo_scene, mvp, write_png, Color, CpuPipeline, Framebuffer, PipelineConfig,
    Topology, Viewport,
};

fn output_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.push("target");
    dir.push("render-output");
    dir
}

/// Renderiza la escena de referencia con el pipeline CPU por etapas.
fn render_scene(width: u32, height: u32, config: PipelineConfig) -> (Framebuffer, String) {
    let mut fb = Framebuffer::new(width, height).expect("valid framebuffer");
    fb.clear(Color::new(8, 10, 18, 255));
    let viewport = Viewport::from_size(width, height).expect("valid viewport");
    let camera = demo_camera(width as f32 / height as f32).expect("valid camera");
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, config);
    for (mesh, transform) in demo_scene() {
        pipeline
            .draw_mesh(&mesh, mvp(&camera, transform), Topology::TriangleList)
            .expect("mesh draws");
    }
    let stats = pipeline.stats;
    let m = &stats.metrics;
    let report = format!(
        "draws={} verts_in={} verts_out={} tris_assembled={} tris_clipped={} tris_culled={} \
tris_rejected={} tris_degenerate={}\n\
fragments_rasterized={} fragments_shaded={} depth_rejected={} pixels_written={} errors={}\n\
stage ns: vertex={} assembly={} clipping={} raster={} fragment={} depth={} total={}",
        stats.draws,
        m.vertices_in,
        m.vertices_out,
        m.triangles_assembled,
        m.triangles_clipped,
        m.triangles_culled,
        m.triangles_rejected,
        m.triangles_degenerate,
        m.fragments_rasterized,
        m.fragments_shaded,
        m.fragments_depth_rejected,
        m.pixels_written,
        stats.errors,
        m.timings.vertex_ns,
        m.timings.assembly_ns,
        m.timings.clipping_ns,
        m.timings.raster_ns,
        m.timings.fragment_ns,
        m.timings.depth_ns,
        m.timings.total_ns(),
    );
    (fb, report)
}

fn non_background_pixels(fb: &Framebuffer) -> usize {
    let bytes = fb.color_bytes();
    bytes
        .chunks_exact(4)
        .filter(|chunk| *chunk != [8, 10, 18, 255])
        .count()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir)?;

    let width = 640u32;
    let height = 480u32;

    let config = PipelineConfig {
        depth_test: true,
        depth_write: true,
        cull_degenerate: true,
        timing: true,
    };
    let (frame, report) = render_scene(width, height, config);
    println!("{report}");
    let path = dir.join("pipeline_scene.png");
    write_png(&path, &frame)?;
    let pixels = non_background_pixels(&frame);
    println!(
        "wrote {} ({} non background pixels)",
        path.display(),
        pixels
    );
    if pixels == 0 {
        return Err("pipeline_scene.png is empty".into());
    }

    // Segunda pasada girada para comprobar la estabilidad del pipeline.
    let mut fb = Framebuffer::new(width, height).expect("valid framebuffer");
    fb.clear(Color::new(8, 10, 18, 255));
    let viewport = Viewport::from_size(width, height).expect("valid viewport");
    let camera = demo_camera(width as f32 / height as f32).expect("valid camera");
    let mut pipeline = CpuPipeline::new(&mut fb, viewport, PipelineConfig::default());
    for (mesh, transform) in demo_scene() {
        let rotated = Transform::new(
            transform.translation,
            transform.rotation * glam::Quat::from_rotation_y(0.35),
            transform.scale,
        );
        pipeline
            .draw_mesh(&mesh, mvp(&camera, rotated), Topology::TriangleList)
            .expect("mesh draws");
    }
    let path = dir.join("pipeline_scene_rotated.png");
    write_png(&path, &fb)?;
    let pixels = non_background_pixels(&fb);
    println!(
        "wrote {} ({} non background pixels)",
        path.display(),
        pixels
    );
    if pixels == 0 {
        return Err("pipeline_scene_rotated.png is empty".into());
    }

    Ok(())
}
