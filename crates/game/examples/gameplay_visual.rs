#[path = "visual_support/mod.rs"]
mod support;

use std::error::Error;
use std::cell::RefCell;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use game::render_backend::{select_backend, BackendDecision, BackendRequest};
use render_core::{CullingConfig, LightingMode, MaterialInfo, MeshHandle, SnapshotScene, VisualAssets};
use render_gpu::{GpuContext, GpuRenderer, OffscreenTarget};

fn print_cpu_timing(label: &str, preparation_ns: u64, stats: &render_cpu::PipelineStats) {
    let timings = stats.metrics.timings;
    println!(
        "cpu-profile label={label} preparation_ms={:.3} vertex_ms={:.3} clip_position_ms={:.3} world_position_ms={:.3} normal_ms={:.3} shading_ms={:.3} vertex_write_ms={:.3} shade_calls={} light_calls={} directional_lights={} point_lights={} point_distance_calculations={} point_normalizations={} shade_inputs_ms={:.3} shade_normal_ms={:.3} shade_view_ms={:.3} shade_light_direction_ms={:.3} shade_attenuation_ms={:.3} shade_diffuse_ms={:.3} shade_specular_ms={:.3} shade_ambient_ms={:.3} shade_composition_ms={:.3} assembly_ms={:.3} clipping_ms={:.3} clip_triangles_received={} clip_trivial_rejects={} clip_trivial_accepts={} clip_full_calls={} clip_plane_distance_evaluations={} clip_plane_intersections={} clip_interpolated_vertices={} clip_final_polygon_vertices={} raster_ms={:.3} fragment_ms={:.3} depth_ms={:.3} pipeline_timed_ms={:.3}",
        preparation_ns as f64 / 1_000_000.0,
        timings.vertex_ns as f64 / 1_000_000.0,
        timings.vertex_clip_ns as f64 / 1_000_000.0,
        timings.vertex_world_ns as f64 / 1_000_000.0,
        timings.vertex_normal_ns as f64 / 1_000_000.0,
        timings.vertex_shading_ns as f64 / 1_000_000.0,
        timings.vertex_write_ns as f64 / 1_000_000.0,
        timings.shade.calls,
        timings.shade.light_calls,
        timings.shade.directional_lights,
        timings.shade.point_lights,
        timings.shade.point_distance_calculations,
        timings.shade.point_normalizations,
        timings.shade.inputs_ns as f64 / 1_000_000.0,
        timings.shade.normal_ns as f64 / 1_000_000.0,
        timings.shade.view_ns as f64 / 1_000_000.0,
        timings.shade.light_direction_ns as f64 / 1_000_000.0,
        timings.shade.attenuation_ns as f64 / 1_000_000.0,
        timings.shade.diffuse_ns as f64 / 1_000_000.0,
        timings.shade.specular_ns as f64 / 1_000_000.0,
        timings.shade.ambient_ns as f64 / 1_000_000.0,
        timings.shade.composition_ns as f64 / 1_000_000.0,
        timings.assembly_ns as f64 / 1_000_000.0,
        timings.clipping_ns as f64 / 1_000_000.0,
        stats.metrics.clipping.triangles_received,
        stats.metrics.clipping.trivial_rejects,
        stats.metrics.clipping.trivial_accepts,
        stats.metrics.clipping.full_clip_calls,
        stats.metrics.clipping.plane_distance_evaluations,
        stats.metrics.clipping.plane_intersections,
        stats.metrics.clipping.interpolated_vertices,
        stats.metrics.clipping.final_polygon_vertices,
        timings.raster_ns as f64 / 1_000_000.0,
        timings.fragment_ns as f64 / 1_000_000.0,
        timings.depth_ns as f64 / 1_000_000.0,
        timings.total_ns() as f64 / 1_000_000.0,
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut backend = String::from("cpu");
    let mut window = false;
    let mut frames = None;
    let mut output = None;
    let mut shader_time = None;
    let mut profile_cpu = false;
    let mut demo_flight = false;
    let mut demo_impact = false;
    let mut graphics_debug = false;
    let mut args = std::env::args().skip(1);
    while let Some(option) = args.next() {
        match option.as_str() {
            "--window" => window = true,
            "--graphics-debug" | "--graphics-validate" => graphics_debug = true,
            "--backend" => backend = args.next().ok_or("missing backend")?,
            "--frames" => {
                frames = Some(
                    args.next()
                        .ok_or("missing frame count")?
                        .parse::<NonZeroU64>()?,
                )
            }
            "--output" => output = Some(PathBuf::from(args.next().ok_or("missing PNG path")?)),
            "--shader-time" => {
                shader_time = Some(args.next().ok_or("missing shader time")?.parse::<f32>()?)
            }
            "--profile-cpu" => profile_cpu = true,
            "--demo-flight" => demo_flight = true,
            "--demo-impact" => demo_impact = true,
            "--help" => {
                println!(
                    "gameplay_visual [--backend cpu|gpu|auto] [--window] [--graphics-validate] [--frames N] [--output PATH] [--shader-time T] [--profile-cpu] [--demo-flight|--demo-impact]"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option {option}").into()),
        }
    }
    if demo_flight && demo_impact {
        return Err("--demo-flight and --demo-impact are mutually exclusive".into());
    }
    let requested = BackendRequest::parse(&backend)?;
    let decision = select_backend(requested, || {
        GpuContext::new(Default::default())
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    println!(
        "backend requested={} selected={} reason={}",
        backend,
        decision.selected(),
        decision.reason().unwrap_or("none")
    );
    backend = decision.selected().into();
    if window && demo_impact {
        return Err("--demo-impact uses offscreen mode; omit --window".into());
    }
    if window && output.is_some() {
        return Err("PNG output uses offscreen mode; omit --window".into());
    }
    let load_started = Instant::now();
    let shooter_mesh = render_core::load_obj(shooter_obj_path())
        .map_err(|error| format!("failed to load Shooter OBJ: {error}"))?;
    println!(
        "shooter-load elapsed_ms={:.3} vertices={} indices={} triangles={} approx_bytes={}",
        load_started.elapsed().as_secs_f64() * 1000.0,
        shooter_mesh.vertices.len(),
        shooter_mesh.indices.len(),
        shooter_mesh.indices.len() / 3,
        shooter_mesh.vertices.len() * std::mem::size_of::<render_core::RasterVertex>()
            + shooter_mesh.indices.len() * std::mem::size_of::<u32>()
    );
    if window {
        let source: render_present::SnapshotSource = if demo_flight {
            let session = Rc::new(RefCell::new(support::deterministic_flight_session().0));
            let stage = Rc::new(RefCell::new(0usize));
            let source_session = Rc::clone(&session);
            let source_stage = Rc::clone(&stage);
            Box::new(move |width, height| {
                let mut session = source_session.borrow_mut();
                let mut stage = source_stage.borrow_mut();
                if *stage == 1 {
                    session.update([gameplay::GameplayCommand::AdvanceTime {
                        delta: engine::Duration(500),
                    }]);
                }
                let snapshot = support::deterministic_flight_snapshot(
                    &session,
                    width,
                    height,
                    *stage,
                );
                *stage = (*stage + 1).min(2);
                Ok(snapshot)
            })
        } else {
            let world = support::fixture_world();
            println!("F5B read-only world tick={} ship={:?} players={} enemies={} destinations={} collectibles={}",world.time().tick.0,world.navigation().ship_id(),world.players().count(),world.enemies().count(),world.destinations().count(),world.collectibles().count());
            Box::new(move |width, height| Ok(support::snapshot(&world, width, height)))
        };
        let config = render_present::WindowConfig {
            max_frames: frames,
            title: "Astral Echo - CPU/GPU Hot-Swap".into(),
            ..Default::default()
        };
        let start_gpu = matches!(decision, BackendDecision::Gpu);
        let debug = render_present::GraphicsDebugConfig {
            directory: graphics_debug.then(|| PathBuf::from("target/graphics-debug")),
        };
        let mut shooter_mesh = Some(shooter_mesh);
        let report = render_present::run_hot_swap_window_with_input_and_scene(
            config,
            source,
            start_gpu,
            |_, _| {},
            move |scene| {
                scene.register_mesh(
                    MeshHandle::ENEMY_PLACEHOLDER,
                    VisualAssets::ENEMY,
                    shooter_mesh
                        .take()
                        .expect("scene configuration should run once"),
                );
                scene.set_lighting_mode(LightingMode::Lambert);
                scene.set_material(VisualAssets::SUN, MaterialInfo::sun());
                scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
                scene.set_material(VisualAssets::STATION, MaterialInfo::station());
                scene.set_material(VisualAssets::DERELICT, MaterialInfo::derelict());
                scene.set_material(VisualAssets::ASTEROID, MaterialInfo::asteroid());
            },
            debug,
        )?;
        println!(
            "hot-swap transitions={} last_transition_ms={:?} gpu_error={:?}",
            report.transitions, report.last_transition_ms, report.gpu_error
        );
        return Ok(());
    }
    let world = support::fixture_world();
    let tick = world.time().tick;
    println!("F5B read-only world tick={} ship={:?} players={} enemies={} destinations={} collectibles={}",tick.0,world.navigation().ship_id(),world.players().count(),world.enemies().count(),world.destinations().count(),world.collectibles().count());
    let (width, height) = (640, 360);
    let snapshots = if demo_flight {
        support::deterministic_flight_snapshots(width, height)
    } else if demo_impact {
        support::deterministic_impact_snapshots(width, height)
    } else {
        vec![support::snapshot(&world, width, height)]
    };
    if demo_impact {
        validate_impact_snapshots(&snapshots)?;
    }
    let mut scene = SnapshotScene::default();
    let register_started = Instant::now();
    scene.register_mesh(
        MeshHandle::ENEMY_PLACEHOLDER,
        VisualAssets::ENEMY,
        shooter_mesh,
    );
    println!(
        "shooter-register elapsed_ms={:.3}",
        register_started.elapsed().as_secs_f64() * 1000.0
    );
    let update_started = Instant::now();
    scene.update(&snapshots[0])?;
    println!(
        "scene-update elapsed_ms={:.3} nodes={}",
        update_started.elapsed().as_secs_f64() * 1000.0,
        scene.snapshot().nodes.len()
    );
    scene.set_lighting_mode(LightingMode::Lambert);
    scene.set_material(VisualAssets::SUN, MaterialInfo::sun());
    scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
    scene.set_material(VisualAssets::STATION, MaterialInfo::station());
    scene.set_material(VisualAssets::DERELICT, MaterialInfo::derelict());
    scene.set_material(VisualAssets::ASTEROID, MaterialInfo::asteroid());
    let count = frames.map(NonZeroU64::get).unwrap_or(1);
    let path = output.unwrap_or_else(|| {
        if demo_impact {
            PathBuf::from(format!("target/f5b-output/{backend}-impact.png"))
        } else {
            PathBuf::from(format!("target/f5b-output/{backend}.png"))
        }
    });
    let impact_paths = demo_impact.then(|| unique_impact_paths(&path));
    if matches!(decision, BackendDecision::Cpu { .. }) {
        let mut baseline_scene = SnapshotScene::default();
        baseline_scene.update(&snapshots[0])?;
        let mut baseline_framebuffer = render_cpu::Framebuffer::new(width, height)?;
        let baseline_started = Instant::now();
        let (baseline_prepared, baseline_pipeline, baseline_preparation_ns) = if profile_cpu && shader_time.is_none() {
            let (prepared, pipeline, preparation_ns) = render_cpu::render_snapshot_scene_with_timing(
                &mut baseline_framebuffer,
                &baseline_scene,
                render_cpu::RasterMode::Secuencial,
            )?;
            (prepared, pipeline, preparation_ns)
        } else if let Some(time) = shader_time {
            let (prepared, pipeline) = render_cpu::render_snapshot_scene_with_shader_at_time_with_stats(
                &mut baseline_framebuffer,
                &baseline_scene,
                render_cpu::RasterMode::Secuencial,
                render_cpu::FragmentShader::SolarBands,
                time,
            )?;
            (prepared, pipeline, 0)
        } else {
            let (prepared, pipeline) = render_cpu::render_snapshot_scene_with_stats(
                &mut baseline_framebuffer,
                &baseline_scene,
                render_cpu::RasterMode::Secuencial,
            )?;
            (prepared, pipeline, 0)
        };
        println!(
            "cpu-baseline elapsed_ms={:.3} prepared_triangles={} vertices_in={} culled={} degenerate={} fragments={} depth_rejected={} pixels_written={}",
            baseline_started.elapsed().as_secs_f64() * 1000.0,
            baseline_prepared.triangles,
            baseline_pipeline.metrics.vertices_in,
            baseline_pipeline.metrics.triangles_culled,
            baseline_pipeline.metrics.triangles_degenerate,
            baseline_pipeline.metrics.fragments_rasterized,
            baseline_pipeline.metrics.fragments_depth_rejected,
            baseline_pipeline.metrics.pixels_written
        );
        if profile_cpu && shader_time.is_none() {
            print_cpu_timing("placeholder", baseline_preparation_ns, &baseline_pipeline);
        }
        let mut framebuffer = render_cpu::Framebuffer::new(width, height)?;
        let start = Instant::now();
        let iterations = if demo_flight || demo_impact {
            snapshots.len()
        } else {
            count as usize
        };
        let mut frame_times_ms = Vec::with_capacity(iterations);
        for index in 0..iterations {
            if index > 0 && index < snapshots.len() {
                scene.update(&snapshots[index])?;
            }
            let render_started = Instant::now();
            let (prepared, pipeline, preparation_ns) = if profile_cpu && shader_time.is_none() {
                let (prepared, pipeline, preparation_ns) = render_cpu::render_snapshot_scene_with_timing(
                    &mut framebuffer,
                    &scene,
                    render_cpu::RasterMode::Secuencial,
                )?;
                (prepared, pipeline, preparation_ns)
            } else if let Some(time) = shader_time {
                let (prepared, pipeline) = render_cpu::render_snapshot_scene_with_shader_at_time_with_stats(
                    &mut framebuffer,
                    &scene,
                    render_cpu::RasterMode::Secuencial,
                    render_cpu::FragmentShader::SolarBands,
                    time,
                )?;
                (prepared, pipeline, 0)
            } else {
                let (prepared, pipeline) = render_cpu::render_snapshot_scene_with_stats(
                    &mut framebuffer,
                    &scene,
                    render_cpu::RasterMode::Secuencial,
                )?;
                (prepared, pipeline, 0)
            };
            frame_times_ms.push(render_started.elapsed().as_secs_f64() * 1000.0);
            if index == 0 {
                println!(
                    "cpu-shooter-frame elapsed_ms={:.3} considered={} visible={} discarded={} triangles={} vertices_in={} rejected={} clipped={} culled={} degenerate={} fragments={} shaded={} depth_rejected={} pixels_written={}",
                    render_started.elapsed().as_secs_f64() * 1000.0,
                    prepared.considered,
                    prepared.visible,
                    prepared.discarded,
                    prepared.triangles,
                    pipeline.metrics.vertices_in,
                    pipeline.metrics.triangles_rejected,
                    pipeline.metrics.triangles_clipped,
                    pipeline.metrics.triangles_culled,
                    pipeline.metrics.triangles_degenerate,
                    pipeline.metrics.fragments_rasterized,
                    pipeline.metrics.fragments_shaded,
                    pipeline.metrics.fragments_depth_rejected,
                    pipeline.metrics.pixels_written
                );
                if profile_cpu && shader_time.is_none() {
                    print_cpu_timing("shooter", preparation_ns, &pipeline);
                }
            }
            if demo_flight {
                render_cpu::write_png(&demo_path(&path, index), &framebuffer)?;
            } else if demo_impact {
                let image = impact_paths
                    .as_ref()
                    .ok_or("missing impact output paths")?[index]
                    .clone();
                let visible = has_game_over_indicator(framebuffer.color_bytes());
                println!(
                    "impact backend=cpu index={index} tick={} fade_to_black={} game_over={} framebuffer_game_over={} path={}",
                    snapshots[index].tick.0,
                    snapshots[index].overlay.fade_to_black,
                    snapshots[index].overlay.game_over,
                    visible,
                    image.display()
                );
                if index == 2 && !visible {
                    return Err("terminal CPU framebuffer has no GAME OVER indicator".into());
                }
                render_cpu::write_png(&image, &framebuffer)?;
            }
        }
        let render_ms = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;
        if !demo_flight {
            render_cpu::write_png(&path, &framebuffer)?;
        }
        let min_ms = frame_times_ms.iter().copied().fold(f64::INFINITY, f64::min);
        let max_ms = frame_times_ms.iter().copied().fold(0.0, f64::max);
        println!("CPU frames={iterations} render_avg_ms={render_ms:.3} measured_min_ms={min_ms:.3} measured_max_ms={max_ms:.3}");
    } else {
        let gpu = Arc::new(GpuContext::new(Default::default())?);
        println!("ADAPTER {:?}", gpu.adapter_info());
        let mut prepared = scene.prepare(CullingConfig {
            width,
            height,
            ..Default::default()
        });
        let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared)?;
        let target = OffscreenTarget::new(gpu.clone(), width, height)?;
        let start = Instant::now();
        let iterations = if demo_flight || demo_impact {
            snapshots.len()
        } else {
            count as usize
        };
        for index in 0..iterations {
            if index > 0 && index < snapshots.len() {
                scene.update(&snapshots[index])?;
                prepared = scene.prepare(CullingConfig {
                    width,
                    height,
                    ..Default::default()
                });
                renderer.sync_prepared(&prepared)?;
            }
            target.render_prepared(&mut renderer, &prepared, scene.view_projection())?;
            gpu.wait()?;
            if demo_flight {
                target.readback()?.write_png(&demo_path(&path, index))?;
            } else if demo_impact {
                let image = target.readback()?;
                let path = impact_paths
                    .as_ref()
                    .ok_or("missing impact output paths")?[index]
                    .clone();
                let visible = has_game_over_indicator(&image.rgba);
                println!(
                    "impact backend=gpu index={index} tick={} fade_to_black={} game_over={} framebuffer_game_over={} path={}",
                    snapshots[index].tick.0,
                    snapshots[index].overlay.fade_to_black,
                    snapshots[index].overlay.game_over,
                    visible,
                    path.display()
                );
                if index == 2 && !visible {
                    return Err("terminal GPU framebuffer has no GAME OVER indicator".into());
                }
                image.write_png(&path)?;
            }
        }
        let render_ms = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;
        let start = Instant::now();
        let image = target.readback()?;
        let readback_ms = start.elapsed().as_secs_f64() * 1000.0;
        println!("GPU frames={iterations} submit_to_completion_avg_ms={render_ms:.3} readback_ms={readback_ms:.3}");
        if !demo_flight && !demo_impact {
            image.write_png("target/f13-shooter-gpu-readback.png")?;
            println!(
                "CPU/GPU comparison {:?}",
                support::compare(&support::cpu_image(&scene, width, height), &image)
            );
        }
        if !demo_flight {
            image.write_png(&path)?;
        }
    }
    if !demo_flight {
        assert_eq!(tick, world.time().tick);
    }
    println!("PNG={} demo_flight={} gameplay_tick_unchanged={}", path.display(), demo_flight, tick.0);
    Ok(())
}

fn shooter_obj_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/shooter/shooter.obj")
}

fn demo_path(path: &Path, index: usize) -> PathBuf {
    let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("frame");
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("png");
    path.with_file_name(format!("{stem}-demo-{index:02}.{extension}"))
}

fn validate_impact_snapshots(
    snapshots: &[render_core::RenderSnapshot],
) -> Result<(), Box<dyn Error>> {
    if snapshots.len() != 3 {
        return Err("impact demo must contain three snapshots".into());
    }
    let before = &snapshots[0].overlay;
    let fade = &snapshots[1].overlay;
    let game_over = &snapshots[2].overlay;
    if before.game_over || before.fade_to_black >= 1.0 {
        return Err("impact demo before snapshot is already terminal".into());
    }
    if !fade.fade_to_black.is_finite()
        || !(0.0 < fade.fade_to_black && fade.fade_to_black < 1.0)
        || fade.game_over
    {
        return Err("impact demo fade snapshot is not partial".into());
    }
    if game_over.fade_to_black != 1.0 || !game_over.game_over {
        return Err("impact demo terminal snapshot is incomplete".into());
    }
    Ok(())
}

fn has_game_over_indicator(rgba: &[u8]) -> bool {
    let mut indicator = false;
    for pixel in rgba.chunks_exact(4) {
        let red = pixel[0] > 200 && pixel[1] < 100 && pixel[2] < 100;
        let black = pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0;
        if red {
            indicator = true;
        } else if !black {
            return false;
        }
    }
    indicator
}

fn unique_impact_paths(path: &Path) -> [PathBuf; 3] {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("impact");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    for suffix in 0.. {
        let marker = if suffix == 0 {
            String::new()
        } else {
            format!("-{suffix}")
        };
        let paths = [
            path.with_file_name(format!("{stem}{marker}-00-before.{extension}")),
            path.with_file_name(format!("{stem}{marker}-01-fade.{extension}")),
            path.with_file_name(format!("{stem}{marker}-02-game-over.{extension}")),
        ];
        if paths.iter().all(|candidate| !candidate.exists()) {
            return paths;
        }
    }
    unreachable!("finite filesystem cannot exhaust impact output suffixes")
}