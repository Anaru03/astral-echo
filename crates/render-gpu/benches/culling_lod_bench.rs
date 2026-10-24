#[path = "../examples/support/mod.rs"]
mod support;

use render_core::{CullingConfig, RenderSnapshot, SnapshotId, SnapshotScene, Vec3};
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
    println!("scene,path,visible,discarded,triangles,prepare_us,frame_ms");
    for (name, count, outside) in [
        ("mixed", 1000, true),
        ("visible", 1000, false),
        ("small", 2, false),
    ] {
        let mut snapshot = RenderSnapshot::minimal_demo();
        let node = snapshot.nodes[0];
        snapshot.nodes.clear();
        for index in 0..count {
            let mut node = node;
            node.entity = SnapshotId::Destination(index);
            node.transform.translation = Vec3::new(
                if outside && index % 5 != 0 {
                    10000.0
                } else {
                    (index % 25) as f32 * 4.0 - 48.0
                },
                0.0,
                -((index / 25) as f32 * 15.0) - 50.0,
            );
            snapshot.nodes.push(node);
        }
        let mut fixture = SnapshotScene::procedural_lod_fixture();
        fixture.update(&snapshot)?;
        let mut f6 = SnapshotScene::default();
        f6.update(&snapshot)?;
        let target = OffscreenTarget::new(gpu.clone(), 320, 180)?;
        let mut reference_image = None;
        for path in ["f6", "f7", "reference", "culling", "culling_lod"] {
            let scene = if path == "f6" || path == "f7" {
                &f6
            } else {
                &fixture
            };
            let reference = path == "f6" || path == "reference";
            let config = CullingConfig {
                width: 320,
                height: 180,
                low_threshold_px: if path == "culling" { 0.0 } else { 12.0 },
                high_threshold_px: if path == "culling" { 0.0 } else { 18.0 },
            };
            let prepared = scene.prepare(config);
            let mut renderer = if reference {
                GpuRenderer::from_snapshot(gpu.clone(), scene)?
            } else {
                GpuRenderer::from_prepared(gpu.clone(), &prepared)?
            };
            let mut preparation = 0.0;
            let mut frame = 0.0;
            for iteration in 0..65 {
                let start = Instant::now();
                let prepare_start = Instant::now();
                if reference {
                    renderer.sync_snapshot(scene)?;
                    if iteration >= 5 {
                        preparation += prepare_start.elapsed().as_secs_f64() * 1e6;
                    }
                    target.render_matrix(&mut renderer, scene.view_projection())?;
                } else {
                    let prepared = scene.prepare(config);
                    renderer.sync_prepared(&prepared)?;
                    if iteration >= 5 {
                        preparation += prepare_start.elapsed().as_secs_f64() * 1e6;
                    }
                    target.render_matrix(&mut renderer, scene.view_projection())?;
                }
                gpu.wait()?;
                if iteration >= 5 {
                    frame += start.elapsed().as_secs_f64() * 1000.0;
                }
            }
            let image = target.readback()?;
            if let Some(reference) = &reference_image {
                let reference: &render_gpu::GpuImage = reference;
                let comparison = support::compare(reference, &image);
                println!("comparison {name}/{path}: {comparison:?}");
                assert!(comparison.passes(), "{comparison:?}");
            } else {
                reference_image = Some(image);
            }
            let camera = render_core::Camera::demo();
            let draws = if reference {
                scene.owned_draws()
            } else {
                prepared.owned_draws()
            };
            let cpu = support::cpu_image(&draws, &camera, 320, 180);
            let gpu_image = target.readback()?;
            let comparison = support::compare(&cpu, &gpu_image);
            println!("CPU/GPU {name}/{path}: {comparison:?}");
            assert!(comparison.passes(), "{comparison:?}");
            let (visible, discarded, triangles) = if reference {
                (
                    count as usize,
                    0,
                    scene.draws().map(|(_, m)| m.indices.len() / 3).sum(),
                )
            } else {
                (prepared.visible, prepared.discarded, prepared.triangles)
            };
            println!(
                "{name},{path},{visible},{discarded},{triangles},{:.3},{:.3}",
                preparation / 60.0,
                frame / 60.0
            );
        }
    }
    Ok(())
}
