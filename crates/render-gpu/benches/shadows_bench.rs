use render_core::{
    shadow_fixture, CullingConfig, LightingMode, LodLevel, Mat4, PreparedScene, RasterMesh,
    RenderSnapshot, ShadowResolution, SnapshotId, SnapshotScene, Vec3, Vec4,
};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};
use std::sync::Arc;
use std::time::Instant;

type BenchResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn main() -> BenchResult {
    if std::env::args().any(|arg| arg == "--cpu-only") {
        return cpu_benchmark();
    }
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            println!("SKIP GPU: {reason}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    println!("ADAPTER {:?}; warmup=8 samples=40 target=320x240, host includes sync+submit+wait, timestamp readback excluded",gpu.adapter_info());
    println!("scene,shadow,pcf,objects,color_draws,shadow_draws,map_bytes,buffer_bytes,map_allocations,buffer_rebuilds,uploads,dynamic_bytes,prepare_us,host_frame_ms,shadow_gpu_ms,color_gpu_ms,cold_ms");
    for name in ["occlusion", "visible1000", "mixed_churn"] {
        let mut scene = fixture(name);
        let vp = Mat4::perspective_rh(1.0, 320.0 / 240.0, 0.1, 100.0)
            * Mat4::look_at_rh(scene.lighting.camera_position, Vec3::ZERO, Vec3::Y);
        scene.lighting.mode = LightingMode::Unlit;
        let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &scene)?;
        let target = OffscreenTarget::new(gpu.clone(), 320, 240)?;
        let timing = renderer.enable_gpu_timing();
        println!("timestamp_supported={timing}");
        scene.lighting.mode = LightingMode::Lambert;
        let cold = Instant::now();
        target.render_prepared(&mut renderer, &scene, vp)?;
        gpu.wait()?;
        println!(
            "cold_lighting {name}: total_ms={:.3} shader_pipeline_validation_ms={:?}",
            cold.elapsed().as_secs_f64() * 1000.0,
            renderer.lighting_cold_setup_ms()
        );
        let all: Vec<_> = scene.draws().cloned().collect();
        let low = Arc::new(RasterMesh::cube(Vec3::ZERO, 0.7, Vec4::ONE));
        let mut expected_depth = None;
        for (resolution, enabled, pcf) in [
            (ShadowResolution::R512, false, false),
            (ShadowResolution::R512, true, false),
            (ShadowResolution::R1024, true, false),
            (ShadowResolution::R2048, true, false),
            (ShadowResolution::R512, true, true),
            (ShadowResolution::R1024, true, true),
            (ShadowResolution::R2048, true, true),
        ] {
            scene.lighting.shadows.enabled = enabled;
            scene.lighting.shadows.resolution = resolution;
            scene.lighting.shadows.pcf = pcf;
            let cold = Instant::now();
            target.render_prepared(&mut renderer, &scene, vp)?;
            gpu.wait()?;
            let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
            let mut prep = 0.0;
            let mut host = 0.0;
            let mut shadow = if timing { 0.0 } else { f64::NAN };
            let mut color = if timing { 0.0 } else { f64::NAN };
            let mut before = None;
            for frame in 0..48 {
                let start = Instant::now();
                if name == "mixed_churn" {
                    let draws = all
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i == 0 || (*i + frame) % 4 != 0)
                        .map(|(i, draw)| {
                            let mut draw = draw.clone();
                            if i != 0 && frame % 2 != 0 {
                                draw.lod = LodLevel::Low;
                                draw.mesh = low.clone();
                            }
                            draw
                        })
                        .collect();
                    let environment = scene.lighting.clone();
                    scene = PreparedScene::from_draws(draws);
                    scene.lighting = environment;
                }
                renderer.sync_prepared(&scene)?;
                let preparation = start.elapsed().as_secs_f64() * 1e6;
                target.render_matrix(&mut renderer, vp)?;
                gpu.wait()?;
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                let times = renderer.read_gpu_timings()?;
                if frame == 7 {
                    before = Some(renderer.resources());
                }
                if frame >= 8 {
                    prep += preparation;
                    host += elapsed;
                    if let Some(times) = times {
                        shadow += times.shadow_ms;
                        color += times.color_ms;
                    }
                }
            }
            let after = renderer.resources();
            let before = before.unwrap();
            assert_eq!(after.geometry_uploads, before.geometry_uploads);
            assert_eq!(after.buffer_rebuilds, before.buffer_rebuilds);
            assert_eq!(after.shadow_map_allocations, before.shadow_map_allocations);
            let image = target.readback()?;
            if let Some(depth) = &expected_depth {
                assert_eq!(&image.depth, depth);
            } else {
                expected_depth = Some(image.depth);
            }
            println!(
                "{name},{},{pcf},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.4},{:.4},{cold_ms:.3}",
                if enabled { resolution as u32 } else { 0 },
                scene.visible,
                after.draw_count,
                after.shadow_draw_count,
                after.shadow_map_bytes,
                after.vertex_bytes + after.index_bytes + after.uniform_bytes + after.instance_bytes,
                after.shadow_map_allocations,
                after.buffer_rebuilds - before.buffer_rebuilds,
                after.geometry_uploads - before.geometry_uploads,
                after.dynamic_bytes_written - before.dynamic_bytes_written,
                prep / 40.0,
                host / 40.0,
                shadow / 40.0,
                color / 40.0
            );
        }
    }
    churn_diagnostic(&gpu)?;
    cpu_benchmark()?;
    Ok(())
}

fn fixture(name: &str) -> PreparedScene {
    let base = shadow_fixture();
    if name == "occlusion" {
        return base;
    }
    let mut draws = vec![base.draws().next().unwrap().clone()];
    let template = base.draws().nth(1).unwrap();
    for id in 0..1000 {
        let mut draw = template.clone();
        draw.node.entity = SnapshotId::Destination(id + 1);
        draw.node.transform.scale = Vec3::splat(0.13);
        draw.node.transform.translation = Vec3::new(
            (id % 40) as f32 * 0.17 - 3.3,
            0.065,
            (id / 40) as f32 * 0.23 - 2.8,
        );
        draws.push(draw);
    }
    let mut scene = PreparedScene::from_draws(draws);
    scene.lighting = base.lighting;
    scene
}

fn churn_diagnostic(gpu: &Arc<GpuContext>) -> BenchResult {
    println!("unlit_churn,repeat,cpu_interleave,update_us,prepare_us,sync_us,submit_us,wait_us,color_gpu_ms,frame_ms,dynamic_bytes,rebuilds");
    for interleave in [false, true] {
        for repeat in 0..3 {
            let mut snapshot = RenderSnapshot::minimal_demo();
            let template = snapshot.nodes[0];
            snapshot.nodes.clear();
            snapshot.camera_position = Vec3::ZERO;
            snapshot.view = Mat4::IDENTITY;
            snapshot.projection = Mat4::perspective_rh(1.0, 320.0 / 180.0, 0.1, 1000.0);
            for id in 0..1000 {
                let mut node = template;
                node.entity = SnapshotId::Destination(id);
                node.transform.translation =
                    Vec3::new((id % 40) as f32 - 19.5, (id / 40) as f32 - 12.0, -80.0);
                snapshot.nodes.push(node);
            }
            let mut scene = SnapshotScene::procedural_lod_fixture();
            let config = CullingConfig {
                width: 320,
                height: 180,
                low_threshold_px: 5.0,
                high_threshold_px: 9.0,
            };
            scene.update(&snapshot)?;
            let prepared = scene.prepare(config);
            let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared)?;
            renderer.enable_gpu_timing();
            let target = OffscreenTarget::new(gpu.clone(), 320, 180)?;
            let mut cpu = render_cpu::Framebuffer::new(320, 180)?;
            let mut sums = [0.0; 7];
            let mut before = None;
            for frame in 0..38 {
                for (i, node) in snapshot.nodes.iter_mut().enumerate() {
                    node.visible = (i + frame) % 4 != 0;
                    node.transform.scale = Vec3::splat(if frame % 2 == 0 { 0.2 } else { 2.0 });
                }
                let start = Instant::now();
                scene.update(&snapshot)?;
                let a = start.elapsed();
                let prepared = scene.prepare(config);
                let b = start.elapsed();
                renderer.sync_prepared(&prepared)?;
                let c = start.elapsed();
                target.render_matrix(&mut renderer, scene.view_projection())?;
                let d = start.elapsed();
                gpu.wait()?;
                let e = start.elapsed();
                let times = renderer.read_gpu_timings()?;
                if interleave {
                    render_cpu::render_prepared_scene(
                        &mut cpu,
                        &prepared,
                        scene.view_projection(),
                        render_cpu::RasterMode::Secuencial,
                    )?;
                }
                if frame == 7 {
                    before = Some(renderer.resources());
                }
                if frame >= 8 {
                    let values = [
                        a.as_secs_f64() * 1e6,
                        (b - a).as_secs_f64() * 1e6,
                        (c - b).as_secs_f64() * 1e6,
                        (d - c).as_secs_f64() * 1e6,
                        (e - d).as_secs_f64() * 1e6,
                        times.map_or(f64::NAN, |t| t.color_ms),
                        e.as_secs_f64() * 1000.0,
                    ];
                    for (sum, value) in sums.iter_mut().zip(values) {
                        *sum += value;
                    }
                }
            }
            let after = renderer.resources();
            let before = before.unwrap();
            println!(
                "unlit_churn,{repeat},{interleave},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.3},{},{}",
                sums[0] / 30.0,
                sums[1] / 30.0,
                sums[2] / 30.0,
                sums[3] / 30.0,
                sums[4] / 30.0,
                sums[5] / 30.0,
                sums[6] / 30.0,
                after.dynamic_bytes_written - before.dynamic_bytes_written,
                after.buffer_rebuilds - before.buffer_rebuilds
            );
        }
    }
    Ok(())
}

fn cpu_benchmark() -> BenchResult {
    println!("cpu_scene,shadow_setting,frame_ms,framebuffer_bytes");
    for name in ["occlusion", "visible1000"] {
        let mut scene = fixture(name);
        let vp = Mat4::perspective_rh(1.0, 320.0 / 240.0, 0.1, 100.0)
            * Mat4::look_at_rh(scene.lighting.camera_position, Vec3::ZERO, Vec3::Y);
        let mut framebuffer = render_cpu::Framebuffer::new(320, 240)?;
        let mut reference = None;
        for enabled in [false, true] {
            scene.lighting.shadows.enabled = enabled;
            let mut elapsed = 0.0;
            for frame in 0..48 {
                let start = Instant::now();
                render_cpu::render_prepared_scene(
                    &mut framebuffer,
                    &scene,
                    vp,
                    render_cpu::RasterMode::Secuencial,
                )?;
                if frame >= 8 {
                    elapsed += start.elapsed().as_secs_f64() * 1000.0;
                }
            }
            if let Some(expected) = reference.as_ref() {
                assert_eq!(expected, &framebuffer);
            } else {
                reference = Some(framebuffer.clone());
            }
            println!(
                "cpu_{name},{enabled},{:.3},{}",
                elapsed / 40.0,
                320 * 240 * 8
            );
        }
    }
    Ok(())
}
