use render_core::{CullingConfig, Mat4, RenderSnapshot, SnapshotId, SnapshotScene, Vec3};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};
use std::sync::Arc;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            println!("SKIP GPU: {reason}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    println!("ADAPTER {:?}", gpu.adapter_info());
    println!("scene,path,draws_avg,buffer_rebuilds,geometry_uploads,dynamic_bytes,peak_buffer_bytes,prepare_us,frame_ms,cpu_ms,cpu_scratch_bytes,assembly_cache_hits");
    for (name, count) in [
        ("small", 2),
        ("visible", 1000),
        ("mixed", 1000),
        ("churn", 1000),
    ] {
        let mut snapshot = RenderSnapshot::minimal_demo();
        let template = snapshot.nodes[0];
        snapshot.nodes.clear();
        snapshot.view = Mat4::IDENTITY;
        snapshot.camera_position = Vec3::ZERO;
        snapshot.projection = Mat4::perspective_rh(1.0, 320.0 / 180.0, 0.1, 1000.0);
        for index in 0..count {
            let mut node = template;
            node.entity = SnapshotId::Destination(index);
            node.transform.scale = Vec3::splat(0.2);
            node.transform.translation = Vec3::new(
                (index % 40) as f32 - 19.5,
                (index / 40) as f32 - 12.0,
                -80.0,
            );
            if name == "mixed" && index % 5 != 0 {
                node.transform.translation.x = 10000.0;
            }
            snapshot.nodes.push(node);
        }
        let mut reference_image: Option<render_gpu::GpuImage> = None;
        let mut reference_cpu = None;
        for path in ["f7a_reference", "instanced"] {
            let mut scene = SnapshotScene::procedural_lod_fixture();
            let config = CullingConfig {
                width: 320,
                height: 180,
                low_threshold_px: 5.0,
                high_threshold_px: 9.0,
            };
            scene.update(&snapshot)?;
            let prepared = scene.prepare(config);
            let mut renderer = GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &[])?;
            if path == "instanced" {
                renderer.sync_prepared(&prepared)?;
            } else {
                renderer.sync_prepared_reference(&prepared)?;
            }
            let target = OffscreenTarget::new(gpu.clone(), 320, 180)?;
            let mut preparation = 0.0;
            let mut frame = 0.0;
            let mut cpu_ms = 0.0;
            let mut draws = 0;
            let mut rebuilds = 0;
            let mut uploads = 0;
            let mut dynamic_bytes = 0;
            let mut peak = 0;
            let mut scratch = 0;
            let mut cache_hits = 0;
            let mut framebuffer = render_cpu::Framebuffer::new(320, 180)?;
            for iteration in 0..35 {
                if name == "churn" {
                    for (index, node) in snapshot.nodes.iter_mut().enumerate() {
                        node.visible = (index + iteration) % 4 != 0;
                        node.transform.scale =
                            Vec3::splat(if iteration % 2 == 0 { 0.2 } else { 2.0 });
                    }
                }
                let previous = renderer.resources();
                let start = Instant::now();
                let prep_start = Instant::now();
                scene.update(&snapshot)?;
                let prepared = scene.prepare(config);
                if path == "instanced" {
                    renderer.sync_prepared(&prepared)?;
                } else {
                    renderer.sync_prepared_reference(&prepared)?;
                }
                let prep = prep_start.elapsed();
                target.render_matrix(&mut renderer, scene.view_projection())?;
                gpu.wait()?;
                let elapsed = start.elapsed();
                let cpu_start = Instant::now();
                let (cpu_stats, cpu_scratch) = draw_cpu(
                    &mut framebuffer,
                    &prepared,
                    scene.view_projection(),
                    path == "instanced",
                )?;
                if iteration >= 5 {
                    preparation += prep.as_secs_f64() * 1e6;
                    frame += elapsed.as_secs_f64() * 1000.0;
                    cpu_ms += cpu_start.elapsed().as_secs_f64() * 1000.0;
                    scratch = scratch.max(cpu_scratch);
                    cache_hits += cpu_stats.assembly_cache_hits;
                    let stats = renderer.resources();
                    draws += stats.draw_count;
                    rebuilds += stats.buffer_rebuilds - previous.buffer_rebuilds;
                    uploads += stats.geometry_uploads - previous.geometry_uploads;
                    dynamic_bytes += stats.dynamic_bytes_written - previous.dynamic_bytes_written;
                    peak = peak.max(
                        stats.vertex_bytes
                            + stats.index_bytes
                            + stats.uniform_bytes
                            + stats.instance_bytes,
                    );
                }
            }
            let image = target.readback()?;
            if let Some(expected) = &reference_image {
                assert_eq!(image.rgba, expected.rgba);
                assert_eq!(image.depth, expected.depth);
                assert_eq!(reference_cpu.as_ref(), Some(&framebuffer));
            }
            let color_outliers = framebuffer
                .color_bytes()
                .chunks_exact(4)
                .zip(image.rgba.chunks_exact(4))
                .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 2))
                .count();
            let depth_outliers = framebuffer
                .depth_slice_mut()
                .iter()
                .zip(&image.depth)
                .filter(|(a, b)| (*a - *b).abs() > 0.00001)
                .count();
            assert!(
                color_outliers <= 288 && depth_outliers <= 288,
                "{name}/{path}: color {color_outliers}, depth {depth_outliers}"
            );
            println!("comparison {name}/{path}: color_outliers={color_outliers}, depth_outliers={depth_outliers}");
            if path == "f7a_reference" {
                reference_image = Some(image);
                reference_cpu = Some(framebuffer.clone());
            }
            println!(
                "{name},{path},{:.2},{rebuilds},{uploads},{dynamic_bytes},{peak},{:.3},{:.3},{:.3},{scratch},{:.1}",
                draws as f64 / 30.0,
                preparation / 30.0,
                frame / 30.0,
                cpu_ms / 30.0,
                cache_hits as f64 / 30.0
            );
        }
    }
    Ok(())
}

fn draw_cpu(
    framebuffer: &mut render_cpu::Framebuffer,
    scene: &render_core::PreparedScene,
    vp: Mat4,
    shared: bool,
) -> Result<(render_cpu::PipelineStats, usize), render_cpu::RasterError> {
    let viewport = render_cpu::Viewport::from_size(framebuffer.width(), framebuffer.height())?;
    let mut pipeline =
        render_cpu::CpuPipeline::new(framebuffer, viewport, render_cpu::PipelineConfig::default())
            .with_tile_config(render_cpu::TileConfig {
                tile_size: 16,
                mode: render_cpu::RasterMode::Secuencial,
            });
    pipeline.clear(render_cpu::Color::new(8, 10, 18, 255));
    if shared {
        pipeline.draw_prepared_scene(scene, vp)?;
    } else {
        for draw in scene.draws() {
            pipeline.draw_mesh(
                &draw.mesh,
                vp * draw.node.transform.matrix(),
                render_cpu::Topology::TriangleList,
            )?;
        }
    }
    Ok((pipeline.stats, pipeline.scratch_bytes()))
}
