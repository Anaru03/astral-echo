use render_core::{
    LightingMode, Mat4, MaterialInfo, RenderLight, RenderLightKind, RenderSnapshot, SnapshotId,
    SnapshotLight, SnapshotScene, Vec3,
};
use render_cpu::{Framebuffer, RasterMode};
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
    println!(
        "ADAPTER {:?}, cpu_vertex_bytes={}",
        gpu.adapter_info(),
        std::mem::size_of::<render_core::RasterVertex>()
    );
    let vp = Mat4::perspective_rh(1.0, 320.0 / 180.0, 0.1, 1000.0);
    println!("scene,mode,objects,triangles,lights,draws,gpu_buffer_bytes,prepare_us,gpu_frame_ms,cpu_frame_ms,cold_gpu_ms");
    for name in ["fixture", "visible1000", "mixed"] {
        let mut prepared = if name == "fixture" {
            render_core::material_fixture()
        } else {
            object_fixture(name == "mixed", vp)
        };
        prepared.lighting.mode = LightingMode::Unlit;
        let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared)?;
        let target = OffscreenTarget::new(gpu.clone(), 320, 180)?;
        let mut framebuffer = Framebuffer::new(320, 180)?;
        let mut depth_reference = None;
        for mode in [
            LightingMode::Unlit,
            LightingMode::Lambert,
            LightingMode::BlinnPhong,
            LightingMode::Pbr,
        ] {
            prepared.lighting.mode = mode;
            let cold = Instant::now();
            target.render_prepared(&mut renderer, &prepared, vp)?;
            gpu.wait()?;
            let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
            let mut prep = 0.0;
            let mut gpu_ms = 0.0;
            let mut cpu_ms = 0.0;
            for index in 0..24 {
                let start = Instant::now();
                renderer.sync_prepared(&prepared)?;
                let prepared_time = start.elapsed();
                target.render_matrix(&mut renderer, vp)?;
                gpu.wait()?;
                let frame_time = start.elapsed();
                let cpu_start = Instant::now();
                render_cpu::render_prepared_scene(
                    &mut framebuffer,
                    &prepared,
                    vp,
                    RasterMode::Secuencial,
                )?;
                if index >= 4 {
                    prep += prepared_time.as_secs_f64() * 1e6;
                    gpu_ms += frame_time.as_secs_f64() * 1000.0;
                    cpu_ms += cpu_start.elapsed().as_secs_f64() * 1000.0;
                }
            }
            let image = target.readback()?;
            if let Some(depth) = &depth_reference {
                assert_eq!(&image.depth, depth);
            } else {
                depth_reference = Some(image.depth.clone());
            }
            assert!(
                framebuffer
                    .depth_slice_mut()
                    .iter()
                    .zip(&image.depth)
                    .filter(|(a, b)| (*a - *b).abs() > 0.00001)
                    .count()
                    < 288
            );
            if mode != LightingMode::Pbr {
                let outliers = framebuffer
                    .color_bytes()
                    .chunks_exact(4)
                    .zip(image.rgba.chunks_exact(4))
                    .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 2))
                    .count();
                assert!(outliers < 288, "{name}/{mode:?}: {outliers} color outliers");
                println!("comparison {name}/{mode:?}: color_outliers={outliers}");
            }
            let stats = renderer.resources();
            let bytes =
                stats.vertex_bytes + stats.index_bytes + stats.uniform_bytes + stats.instance_bytes;
            println!(
                "{name},{mode:?},{},{},{},{},{bytes},{:.3},{:.3},{:.3},{cold_ms:.3}",
                prepared.visible,
                prepared.triangles,
                prepared.lighting.lights().len(),
                stats.draw_count,
                prep / 20.0,
                gpu_ms / 20.0,
                cpu_ms / 20.0
            );
        }
    }
    Ok(())
}

fn object_fixture(mixed: bool, vp: Mat4) -> render_core::PreparedScene {
    let mut snapshot = RenderSnapshot::minimal_demo();
    let template = snapshot.nodes[0];
    snapshot.nodes.clear();
    snapshot.view = Mat4::IDENTITY;
    snapshot.projection = vp;
    snapshot.camera_position = Vec3::ZERO;
    snapshot.lights.clear();
    snapshot.push_light(SnapshotLight::new(RenderLight::demo_sun()));
    for index in 0..4 {
        snapshot.push_light(SnapshotLight::new(RenderLight {
            color: Vec3::ONE,
            intensity: 100.0,
            kind: RenderLightKind::Point {
                position: Vec3::new(
                    if index % 2 == 0 { -10.0 } else { 10.0 },
                    if index < 2 { -10.0 } else { 10.0 },
                    -70.0,
                ),
                range: 100.0,
            },
            casts_shadow: false,
        }));
    }
    for index in 0..1000 {
        let mut node = template;
        node.entity = SnapshotId::Destination(index);
        node.transform.scale = Vec3::splat(0.2);
        node.transform.translation = Vec3::new(
            if mixed && index % 5 != 0 {
                10000.0
            } else {
                (index % 40) as f32 - 19.5
            },
            (index / 40) as f32 - 12.0,
            -80.0,
        );
        snapshot.nodes.push(node);
    }
    let mut scene = SnapshotScene::procedural_lod_fixture();
    scene.set_material(template.material, MaterialInfo::fixture_rough());
    scene.update(&snapshot).unwrap();
    scene.prepare(render_core::CullingConfig {
        width: 320,
        height: 180,
        ..Default::default()
    })
}
