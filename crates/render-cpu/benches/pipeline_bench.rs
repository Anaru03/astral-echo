use std::path::PathBuf;
use std::time::{Duration, Instant};

use glam::Vec3;
use rayon::ThreadPoolBuilder;
use render_core::Transform;
use render_cpu::{
    demo_camera, demo_scene, grid_mesh, mvp, write_png, Color, CpuPipeline, Framebuffer,
    PipelineConfig, RasterMesh, RasterMode, TileConfig, TileStats, Topology, Viewport,
};

struct Measurement {
    samples: Vec<Duration>,
    bin_ns: u64,
    raster_ns: u64,
    scratch_bytes: usize,
    workers_used: usize,
    parallel_draws: u32,
    sequential_draws: u32,
    estimated_work: u64,
    bin_entries: usize,
}

impl Measurement {
    fn avg_ms(&self) -> f64 {
        self.samples.iter().map(Duration::as_secs_f64).sum::<f64>() * 1000.0
            / self.samples.len() as f64
    }

    fn percentile_ms(&self, percent: usize) -> f64 {
        let mut samples = self.samples.clone();
        samples.sort_unstable();
        let index = (samples.len() * percent).div_ceil(100).saturating_sub(1);
        samples[index].as_secs_f64() * 1000.0
    }
}

fn measure(
    scene: &[(RasterMesh, Transform)],
    size: (u32, u32),
    mode: Option<RasterMode>,
    iterations: usize,
) -> (Measurement, Framebuffer) {
    let camera = demo_camera(size.0 as f32 / size.1 as f32).unwrap();
    let viewport = Viewport::from_size(size.0, size.1).unwrap();
    let mut framebuffer = Framebuffer::new(size.0, size.1).unwrap();
    let mut measurement = Measurement {
        samples: Vec::with_capacity(iterations),
        bin_ns: 0,
        raster_ns: 0,
        scratch_bytes: 0,
        workers_used: 0,
        parallel_draws: 0,
        sequential_draws: 0,
        estimated_work: 0,
        bin_entries: 0,
    };
    {
        let mut pipeline = CpuPipeline::new(
            &mut framebuffer,
            viewport,
            PipelineConfig {
                timing: true,
                ..PipelineConfig::default()
            },
        );
        if let Some(mode) = mode {
            pipeline = pipeline.with_tile_config(TileConfig {
                tile_size: 16,
                mode,
            });
        }
        for iteration in 0..iterations + 3 {
            pipeline.clear(Color::new(8, 10, 18, 255));
            pipeline.stats = Default::default();
            pipeline.tile_stats = TileStats::default();
            let start = Instant::now();
            for (mesh, transform) in scene {
                pipeline
                    .draw_mesh(mesh, mvp(&camera, *transform), Topology::TriangleList)
                    .unwrap();
            }
            let elapsed = start.elapsed();
            assert_eq!(pipeline.stats.errors, 0);
            if iteration < 3 {
                continue;
            }
            measurement.samples.push(elapsed);
            measurement.scratch_bytes = measurement.scratch_bytes.max(pipeline.scratch_bytes());
            measurement.workers_used = measurement
                .workers_used
                .max(pipeline.tile_stats.workers_used.max(1));
            measurement.bin_ns += pipeline.tile_stats.bin_ns;
            measurement.estimated_work += pipeline.tile_stats.estimated_work;
            measurement.bin_entries += pipeline.tile_stats.bin_entries;
            measurement.raster_ns += if mode.is_some() {
                pipeline.tile_stats.raster_ns
            } else {
                let t = pipeline.stats.metrics.timings;
                t.raster_ns + t.fragment_ns + t.depth_ns
            };
            measurement.parallel_draws += pipeline.tile_stats.parallel_draws;
            measurement.sequential_draws += pipeline.tile_stats.sequential_draws;
        }
    }
    (measurement, framebuffer)
}

fn overdraw_scene() -> Vec<(RasterMesh, Transform)> {
    let base = grid_mesh(8, 22.0, 3.0);
    let mut mesh = RasterMesh::default();
    for _ in 0..32 {
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend_from_slice(&base.vertices);
        mesh.indices
            .extend(base.indices.iter().map(|index| index + offset));
    }
    vec![(mesh, Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)))]
}

fn compare(a: &Framebuffer, b: &Framebuffer) -> (usize, usize) {
    let colors = a
        .color_bytes()
        .chunks_exact(4)
        .zip(b.color_bytes().chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    let mut depths = 0;
    for y in 0..a.height() {
        for x in 0..a.width() {
            depths += usize::from(
                a.depth_at(x, y).unwrap().to_bits() != b.depth_at(x, y).unwrap().to_bits(),
            );
        }
    }
    (colors, depths)
}

fn hash(framebuffer: &Framebuffer) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in framebuffer.color_bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    for y in 0..framebuffer.height() {
        for x in 0..framebuffer.width() {
            for byte in framebuffer.depth_at(x, y).unwrap().to_bits().to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
    }
    hash
}

fn main() {
    let iterations = std::env::var("ASTRAL_BENCH_ITERATIONS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .or_else(|| std::env::args().skip(1).find_map(|s| s.parse().ok()))
        .unwrap_or(50)
        .max(1);
    let available = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let workers = std::env::var("ASTRAL_BENCH_WORKERS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(available.min(8))
        .max(1);
    let output = std::env::var_os("ASTRAL_BENCH_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/f42-bench")
        });
    std::fs::create_dir_all(&output).unwrap();
    println!("F4.2; samples={iterations}; warmup=3; pool_workers={workers}; available_workers={available}; os={}; arch={}", std::env::consts::OS, std::env::consts::ARCH);
    println!("scratch_bytes counts renderer-owned vector capacities; excludes framebuffer, input mesh, allocator metadata and Rayon runtime; clear/validation/export excluded from frame time");
    let modes = [
        ("original", None),
        ("tiled_seq", Some(RasterMode::Secuencial)),
        ("tiled_parallel", Some(RasterMode::Paralelo)),
        ("auto", Some(RasterMode::Auto)),
    ];
    let scenes = [
        ("f3", demo_scene()),
        (
            "geometry",
            vec![(
                grid_mesh(128, 22.0, 3.0),
                Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)),
            )],
        ),
        ("overdraw", overdraw_scene()),
    ];
    let mut csv = String::from("scene,width,height,mode,iterations,pool_workers,used_workers,avg_ms,p95_ms,min_ms,max_ms,speedup_original,speedup_tiled_seq,scratch_bytes,delta_scratch_original_bytes,bin_avg_ms,raster_avg_ms,parallel_draws_per_frame,sequential_draws_per_frame,color_mismatches,depth_mismatches,image_depth_hash,estimated_work_per_frame,bin_entries_per_frame\n");
    let mut samples_csv = String::from("scene,width,height,mode,sample,ms\n");
    let pool = ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap();
    pool.install(|| {
        for (scene_name, scene) in &scenes {
            for size in [(320, 180), (640, 360), (1280, 720)] {
                let mut measurements = Vec::new();
                let mut reference = None;
                for (name, mode) in modes {
                    let (measurement, framebuffer) = measure(scene, size, mode, iterations);
                    let (colors, depths) = reference.as_ref().map(|expected| compare(expected, &framebuffer)).unwrap_or((0, 0));
                    assert_eq!((colors, depths), (0, 0), "{scene_name} {size:?} {name}");
                    write_png(&output.join(format!("{scene_name}-{}x{}-{name}.png", size.0, size.1)), &framebuffer).unwrap();
                    let image_hash = hash(&framebuffer);
                    if reference.is_none() { reference = Some(framebuffer); }
                    measurements.push((name, measurement, image_hash));
                }
                let original_ms = measurements[0].1.avg_ms();
                let tiled_ms = measurements[1].1.avg_ms();
                let original_scratch = measurements[0].1.scratch_bytes;
                for (name, m, image_hash) in measurements {
                    let average = m.avg_ms();
                    let row = format!("{scene_name},{},{},{name},{iterations},{workers},{},{average:.6},{:.6},{:.6},{:.6},{:.4},{:.4},{},{},{:.6},{:.6},{:.2},{:.2},0,0,{image_hash:016x},{:.2},{:.2}\n",
                        size.0, size.1, m.workers_used, m.percentile_ms(95), m.percentile_ms(0), m.percentile_ms(100),
                        original_ms / average, tiled_ms / average, m.scratch_bytes, m.scratch_bytes as i128 - original_scratch as i128,
                        m.bin_ns as f64 / iterations as f64 / 1e6, m.raster_ns as f64 / iterations as f64 / 1e6,
                        m.parallel_draws as f64 / iterations as f64, m.sequential_draws as f64 / iterations as f64,
                        m.estimated_work as f64 / iterations as f64, m.bin_entries as f64 / iterations as f64);
                    print!("{row}");
                    csv.push_str(&row);
                    for (index, sample) in m.samples.iter().enumerate() {
                        samples_csv.push_str(&format!("{scene_name},{},{},{name},{index},{:.6}\n", size.0, size.1, sample.as_secs_f64() * 1000.0));
                    }
                }
            }
        }
    });
    std::fs::write(output.join("summary.csv"), csv).unwrap();
    std::fs::write(output.join("samples.csv"), samples_csv).unwrap();
    println!("results={}", output.display());
}
