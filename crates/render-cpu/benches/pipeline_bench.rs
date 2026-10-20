//! Benchmark del pipeline de rasterización CPU.
//!
//! No usa Criterion ni ninguna dependencia externa: el harness es `false` en
//! `Cargo.toml` y las mediciones se hacen con `std::time::Instant`.
//!
//! Uso: `cargo bench -p render-cpu --offline` (200 iteraciones por escenario
//! por defecto; se puede cambiar con un argumento, p. ej. `-- 500`).

use std::time::{Duration, Instant};

use glam::{Vec3, Vec4};
use render_core::Transform;
use render_cpu::{
    demo_camera, demo_scene, grid_mesh, mvp, Color, CpuPipeline, Framebuffer, PipelineConfig,
    RasterMesh, RasterVertex, Stage, StageTimings, Topology, Viewport,
};

/// Resultado de una medición repetida.
struct Measurement {
    name: String,
    iterations: u32,
    width: u32,
    height: u32,
    total: Duration,
    min: Duration,
    max: Duration,
    p95: Duration,
    timings: StageTimings,
    pixels_written: u32,
    fragments_rasterized: u32,
    triangles: u32,
}

impl Measurement {
    fn avg(&self) -> Duration {
        self.total / self.iterations.max(1)
    }

    /// Milisegundos por megapíxel y por frame.
    fn ms_per_megapixel(&self) -> f64 {
        let megapixels = (self.width as f64 * self.height as f64) / 1_000_000.0;
        if megapixels <= 0.0 {
            return 0.0;
        }
        self.avg().as_secs_f64() * 1e3 / megapixels
    }
}

/// Percentil 95 de una lista de duraciones (ordenada ascendentemente).
fn percentile95(samples: &[Duration]) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

/// Ejecuta `iterations` renders de la escena y acumula las métricas.
fn measure(
    name: &str,
    iterations: u32,
    width: u32,
    height: u32,
    scene: &[(RasterMesh, Transform)],
) -> Measurement {
    let viewport = Viewport::from_size(width, height).expect("valid viewport");
    let camera = demo_camera(width as f32 / height as f32).expect("valid camera");
    let mut framebuffer = Framebuffer::new(width, height).expect("valid framebuffer");
    let config = PipelineConfig {
        depth_test: true,
        depth_write: true,
        cull_degenerate: true,
        timing: true,
    };
    let mut pipeline = CpuPipeline::new(&mut framebuffer, viewport, config);

    // Frame de calentamiento: evita medir el coste de la primera asignación.
    pipeline.clear(Color::new(8, 10, 18, 255));
    for (mesh, transform) in scene {
        pipeline
            .draw_mesh(mesh, mvp(&camera, *transform), Topology::TriangleList)
            .expect("warmup draw");
    }
    pipeline.stats = Default::default();

    let mut min = Duration::MAX;
    let mut max = Duration::ZERO;
    let mut total = Duration::ZERO;
    let mut samples: Vec<Duration> = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        pipeline.clear(Color::new(8, 10, 18, 255));
        let start = Instant::now();
        for (mesh, transform) in scene {
            pipeline
                .draw_mesh(mesh, mvp(&camera, *transform), Topology::TriangleList)
                .expect("draw");
        }
        let elapsed = start.elapsed();
        total += elapsed;
        min = min.min(elapsed);
        max = max.max(elapsed);
        samples.push(elapsed);
    }
    let stats = pipeline.stats;
    Measurement {
        name: name.to_string(),
        iterations,
        width,
        height,
        total,
        min,
        max,
        p95: percentile95(&samples),
        timings: stats.metrics.timings,
        pixels_written: stats.metrics.pixels_written,
        fragments_rasterized: stats.metrics.fragments_rasterized,
        triangles: stats.metrics.triangles_assembled,
    }
}

fn report(m: &Measurement) {
    let iters = m.iterations.max(1);
    println!(
        "  {:<26} iters={:<4} avg={:>8.3} ms  p95={:>8.3} ms  min={:>8.3} ms  max={:>8.3} ms  \
ms/Mpx={:>6.3}  tris/f={:<5} px/f={:<7} frag/f={}",
        m.name,
        m.iterations,
        m.avg().as_secs_f64() * 1e3,
        m.p95.as_secs_f64() * 1e3,
        m.min.as_secs_f64() * 1e3,
        m.max.as_secs_f64() * 1e3,
        m.ms_per_megapixel(),
        m.triangles / iters,
        m.pixels_written / iters,
        m.fragments_rasterized / iters,
    );
    let t = &m.timings;
    println!(
        "      ns/frame: vertex={} assembly={} clipping={} raster={} fragment={} depth={} total={}",
        t.vertex_ns / iters as u64,
        t.assembly_ns / iters as u64,
        t.clipping_ns / iters as u64,
        t.raster_ns / iters as u64,
        t.fragment_ns / iters as u64,
        t.depth_ns / iters as u64,
        t.total_ns() / iters as u64,
    );
    let total = t.total_ns().max(1) as f64;
    let mut parts = Vec::new();
    for stage in [
        Stage::Vertex,
        Stage::Assembly,
        Stage::Clipping,
        Stage::Raster,
        Stage::Fragment,
        Stage::Depth,
    ] {
        let ns = t.get(stage).unwrap_or(0);
        parts.push(format!(
            "{}={:.1}%",
            stage.name(),
            ns as f64 / total * 100.0
        ));
    }
    println!("      share: {}", parts.join("  "));
}

fn main() {
    let iterations: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(200);

    println!("Astral Echo — benchmark del pipeline CPU");
    println!("iteraciones por escenario: {iterations}");
    println!("perfil: release, sin GPU, reloj std::time::Instant\n");

    let reference = demo_scene();

    println!("Escena de referencia (terreno + 12 cubos + 6 triangulos):");
    let reference_m = measure("640x360 full scene", iterations, 640, 360, &reference);
    report(&reference_m);

    println!("\nEscalado de resolucion (misma escena):");
    for (w, h) in [(320u32, 180u32), (640, 360), (1280, 720)] {
        let m = measure(&format!("{w}x{h}"), iterations, w, h, &reference);
        report(&m);
    }

    println!("\nEscena minima (1 triangulo): coste puro del pipeline");
    let minimal = vec![(
        RasterMesh::triangle(
            RasterVertex::new(Vec3::new(-0.6, -0.5, 0.0), Vec4::new(1.0, 0.3, 0.2, 1.0)),
            RasterVertex::new(Vec3::new(0.6, -0.5, 0.0), Vec4::new(0.2, 1.0, 0.4, 1.0)),
            RasterVertex::new(Vec3::new(0.0, 0.6, 0.0), Vec4::new(0.4, 0.5, 1.0, 1.0)),
        ),
        Transform::IDENTITY,
    )];
    let m = measure("1 triangle 640x360", iterations, 640, 360, &minimal);
    report(&m);

    println!("\nEscalado de carga (subdivision del terreno, 640x360):");
    for subdivisions in [8u32, 16, 32] {
        let scene = vec![(
            grid_mesh(subdivisions, 22.0, 3.0),
            Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)),
        )];
        let m = measure(
            &format!("grid {subdivisions}x{subdivisions}"),
            iterations,
            640,
            360,
            &scene,
        );
        report(&m);
    }

    println!("\nResumen (escena de referencia 640x360):");
    let avg = reference_m.avg();
    println!(
        "  avg={:.3} ms/frame  p95={:.3} ms  min={:.3} ms  max={:.3} ms  => {:.1} fps (avg)",
        avg.as_secs_f64() * 1e3,
        reference_m.p95.as_secs_f64() * 1e3,
        reference_m.min.as_secs_f64() * 1e3,
        reference_m.max.as_secs_f64() * 1e3,
        1.0 / avg.as_secs_f64(),
    );
}
