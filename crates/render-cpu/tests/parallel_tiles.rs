use glam::Vec4;
use rayon::{ThreadPool, ThreadPoolBuilder};
use render_cpu::{
    demo_camera, demo_scene, mvp, rasterize_triangles_tiled, Color, CpuPipeline, FillRule,
    Framebuffer, PipelineConfig, RasterMode, RasterWorkload, ScreenVertex, TileConfig, TileStats,
    Topology, Viewport,
};

type Triangle = (ScreenVertex, ScreenVertex, ScreenVertex);

fn pool(workers: usize) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap()
}

fn quad(width: u32, height: u32, depth: f32, color: Vec4) -> Vec<Triangle> {
    let vertex = |x, y, inv_w| ScreenVertex {
        x,
        y,
        z: depth,
        inv_w,
        color,
    };
    let a = vertex(0.0, 0.0, 1.0);
    let b = vertex(width as f32, 0.0, 0.5);
    let c = vertex(width as f32, height as f32, 0.25);
    let d = vertex(0.0, height as f32, 0.75);
    vec![(a, b, c), (a, c, d)]
}

fn scene(width: u32, height: u32) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    for layer in 0..4 {
        let color = Vec4::new(layer as f32 / 4.0, 0.5, 1.0, 0.25 + layer as f32 / 8.0);
        let mut vertices = quad(width, height, 0.8 - layer as f32 * 0.1, color);
        if layer % 2 == 0 {
            for (a, b, c) in &mut vertices {
                std::mem::swap(b, c);
                a.color = Vec4::new(1.0, 0.0, 0.25, 0.5);
            }
        }
        triangles.extend(vertices);
    }
    triangles
}

fn render(
    triangles: &[Triangle],
    size: (u32, u32),
    mode: RasterMode,
    depth: (bool, bool),
    tile_size: u32,
) -> (Framebuffer, TileStats) {
    let mut framebuffer = Framebuffer::new(size.0, size.1).unwrap();
    framebuffer.clear(Color::new(13, 23, 31, 255));
    framebuffer.set_depth(1, 1, 0.1);
    let stats = rasterize_triangles_tiled(
        triangles,
        &mut framebuffer,
        FillRule::TopLeft,
        depth.0,
        depth.1,
        TileConfig { tile_size, mode },
    );
    (framebuffer, stats)
}

fn assert_buffers_equal(a: &Framebuffer, b: &Framebuffer) {
    assert_eq!(a.color_bytes(), b.color_bytes());
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    for y in 0..a.height() {
        for x in 0..a.width() {
            assert_eq!(
                a.depth_at(x, y).unwrap().to_bits(),
                b.depth_at(x, y).unwrap().to_bits(),
                "depth at {x},{y}"
            );
        }
    }
}

#[test]
fn parallel_executes_on_multiple_rayon_workers() {
    let triangles = scene(640, 360);
    let (_, stats) = pool(4).install(|| {
        render(
            &triangles,
            (640, 360),
            RasterMode::Paralelo,
            (true, true),
            16,
        )
    });
    assert_eq!(stats.selected_mode, RasterMode::Paralelo);
    assert!(
        (2..=4).contains(&stats.workers_used),
        "workers used: {}",
        stats.workers_used
    );
    assert!(stats.pixels_written > 0);
}

#[test]
fn parallel_matches_large_scenes_and_partial_edge_tiles() {
    let workers = pool(3);
    for size in [(17, 13), (641, 361), (1280, 720)] {
        let triangles = scene(size.0, size.1);
        let (sequential, _) = render(&triangles, size, RasterMode::Secuencial, (true, true), 16);
        let (parallel, _) =
            workers.install(|| render(&triangles, size, RasterMode::Paralelo, (true, true), 16));
        assert_buffers_equal(&sequential, &parallel);
    }
}

#[test]
fn parallel_preserves_depth_test_and_write_combinations() {
    let triangles = scene(65, 37);
    let workers = pool(2);
    for depth in [(false, false), (false, true), (true, false), (true, true)] {
        let (sequential, expected) = render(&triangles, (65, 37), RasterMode::Secuencial, depth, 7);
        let (parallel, actual) =
            workers.install(|| render(&triangles, (65, 37), RasterMode::Paralelo, depth, 7));
        assert_buffers_equal(&sequential, &parallel);
        assert_eq!(
            expected.fragments_depth_rejected,
            actual.fragments_depth_rejected
        );
        assert_eq!(expected.pixels_written, actual.pixels_written);
    }
}

#[test]
fn alpha_write_order_is_stable_across_workers() {
    let red = quad(65, 37, 0.5, Vec4::new(1.0, 0.0, 0.0, 0.5));
    let blue = quad(65, 37, 0.5, Vec4::new(0.0, 0.0, 1.0, 0.25));
    let workers = pool(3);
    for (triangles, expected) in [
        (
            [red.clone(), blue.clone()].concat(),
            Color::new(0, 0, 255, 64),
        ),
        ([blue, red].concat(), Color::new(255, 0, 0, 128)),
    ] {
        let (sequential, _) = render(
            &triangles,
            (65, 37),
            RasterMode::Secuencial,
            (false, false),
            16,
        );
        let (parallel, _) = workers.install(|| {
            render(
                &triangles,
                (65, 37),
                RasterMode::Paralelo,
                (false, false),
                16,
            )
        });
        assert_buffers_equal(&sequential, &parallel);
        assert_eq!(parallel.pixel(2, 2), Some(expected));
    }
}

#[test]
fn parallel_is_deterministic_across_repeated_runs_and_pool_sizes() {
    let triangles = scene(129, 73);
    let (expected, _) = render(
        &triangles,
        (129, 73),
        RasterMode::Secuencial,
        (true, true),
        11,
    );
    for count in [1, 2, 4] {
        let workers = pool(count);
        for _ in 0..5 {
            let (actual, _) = workers.install(|| {
                render(
                    &triangles,
                    (129, 73),
                    RasterMode::Paralelo,
                    (true, true),
                    11,
                )
            });
            assert_buffers_equal(&expected, &actual);
        }
    }
}

#[test]
fn parallel_shared_edges_have_no_double_coverage() {
    let triangles = quad(65, 37, 0.5, Vec4::ONE);
    let (framebuffer, stats) = pool(3).install(|| {
        render(
            &triangles,
            (65, 37),
            RasterMode::Paralelo,
            (false, false),
            16,
        )
    });
    assert_eq!(stats.fragments_rasterized, 65 * 37);
    assert_eq!(stats.pixels_written, 65 * 37);
    assert!(framebuffer.color_bytes().iter().all(|&value| value == 255));
}

#[test]
fn auto_selects_small_large_and_single_worker_scenes() {
    let workers = pool(2);
    for (size, expected) in [
        ((17, 13), RasterMode::Secuencial),
        ((640, 360), RasterMode::Paralelo),
    ] {
        let triangles = scene(size.0, size.1);
        let (reference, _) = render(&triangles, size, RasterMode::Secuencial, (true, true), 16);
        let (actual, stats) =
            workers.install(|| render(&triangles, size, RasterMode::Auto, (true, true), 16));
        assert_eq!(stats.selected_mode, expected);
        assert_buffers_equal(&reference, &actual);
    }
    let triangles = scene(640, 360);
    let (_, stats) =
        pool(1).install(|| render(&triangles, (640, 360), RasterMode::Auto, (true, true), 16));
    assert_eq!(stats.selected_mode, RasterMode::Secuencial);
    assert_eq!(stats.workers_used, 1);
}

#[test]
fn auto_accounts_for_binning_resolution_and_available_bands() {
    let work = RasterWorkload {
        framebuffer_pixels: 640 * 360,
        estimated_work: 2_000_000,
        bin_entries: 1000,
        tile_count: 920,
        active_bands: 23,
        workers: 4,
    };
    assert_eq!(RasterMode::Auto.select(work), RasterMode::Paralelo);
    for changed in [
        RasterWorkload { workers: 1, ..work },
        RasterWorkload {
            active_bands: 1,
            ..work
        },
        RasterWorkload {
            framebuffer_pixels: 64 * 64,
            ..work
        },
        RasterWorkload {
            estimated_work: 500,
            ..work
        },
        RasterWorkload {
            bin_entries: 100_000,
            ..work
        },
    ] {
        assert_eq!(RasterMode::Auto.select(changed), RasterMode::Secuencial);
        assert_eq!(
            RasterMode::Secuencial.select(changed),
            RasterMode::Secuencial
        );
        assert_eq!(RasterMode::Paralelo.select(changed), RasterMode::Paralelo);
    }
}

#[test]
fn empty_parallel_draw_does_not_touch_framebuffer() {
    let (reference, _) = render(&[], (17, 13), RasterMode::Secuencial, (true, true), 0);
    let (actual, stats) =
        pool(2).install(|| render(&[], (17, 13), RasterMode::Paralelo, (true, true), 0));
    assert_buffers_equal(&reference, &actual);
    assert_eq!(stats.workers_used, 0);
    assert_eq!(stats.scratch_bytes, 0);
}

#[test]
fn f3_pipeline_matches_original_in_all_tiled_modes() {
    let size = (320, 180);
    let scene = demo_scene();
    let camera = demo_camera(size.0 as f32 / size.1 as f32).unwrap();
    let draw = |mode| {
        let mut framebuffer = Framebuffer::new(size.0, size.1).unwrap();
        let mut pipeline = CpuPipeline::new(
            &mut framebuffer,
            Viewport::from_size(size.0, size.1).unwrap(),
            PipelineConfig::default(),
        );
        if let Some(mode) = mode {
            pipeline = pipeline.with_tile_config(TileConfig {
                tile_size: 16,
                mode,
            });
        }
        pipeline.clear(Color::BLACK);
        for (mesh, transform) in &scene {
            pipeline
                .draw_mesh(mesh, mvp(&camera, *transform), Topology::TriangleList)
                .unwrap();
        }
        let stats = pipeline.stats;
        let tile_stats = pipeline.tile_stats;
        (framebuffer, stats, tile_stats)
    };
    let (expected, stats, _) = draw(None);
    let workers = pool(3);
    for mode in [
        RasterMode::Secuencial,
        RasterMode::Paralelo,
        RasterMode::Auto,
    ] {
        let (actual, actual_stats, tile_stats) = workers.install(|| draw(Some(mode)));
        assert_buffers_equal(&expected, &actual);
        assert_eq!(stats, actual_stats);
        assert_eq!(tile_stats.bin_ns, 0);
        assert_eq!(tile_stats.raster_ns, 0);
    }
}
