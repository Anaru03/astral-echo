use std::sync::Arc;

use render_core::{RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};

#[test]
fn snapshot_updates_preserve_buffers_until_topology_changes() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let mut snapshot = RenderSnapshot::minimal_demo();
    let mut scene = SnapshotScene::default();
    scene.update(&snapshot).unwrap();
    let mut renderer = GpuRenderer::from_snapshot(gpu.clone(), &scene).unwrap();
    let stats = renderer.resources();
    let target = OffscreenTarget::new(gpu, 321, 181).unwrap();
    target.render_snapshot(&mut renderer, &scene).unwrap();
    let first = target.readback().unwrap();
    snapshot.nodes[0].transform.translation += Vec3::new(8.0, 0.0, -3.0);
    snapshot.nodes.reverse();
    scene.update(&snapshot).unwrap();
    assert!(!renderer.sync_snapshot(&scene).unwrap());
    let moved = renderer.resources();
    assert_eq!(
        (moved.vertex_bytes, moved.index_bytes, moved.uniform_bytes),
        (stats.vertex_bytes, stats.index_bytes, stats.uniform_bytes)
    );
    target.render_snapshot(&mut renderer, &scene).unwrap();
    assert_ne!(target.readback().unwrap().rgba, first.rgba);
    let mut added = SnapshotNode::destination(
        999,
        Transform::from_translation(Vec3::new(-8.0, 0.0, -8.0)),
        render_core::MeshHandle::DESTINATION_PLACEHOLDER,
        render_core::VisualAssets::DESTINATION,
        true,
    );
    snapshot.push_node(added);
    scene.update(&snapshot).unwrap();
    assert!(renderer.sync_snapshot(&scene).unwrap());
    assert_eq!(renderer.resources().draw_count, 3);
    added.visible = false;
    snapshot.push_node(added);
    scene.update(&snapshot).unwrap();
    assert!(renderer.sync_snapshot(&scene).unwrap());
    assert_eq!(renderer.resources().draw_count, 2);
    snapshot.nodes.clear();
    scene.update(&snapshot).unwrap();
    assert!(renderer.sync_snapshot(&scene).unwrap());
    target.render_snapshot(&mut renderer, &scene).unwrap();
    let empty = target.readback().unwrap();
    assert!(empty
        .rgba
        .chunks_exact(4)
        .all(|pixel| pixel == render_gpu::CLEAR_RGBA));
    assert!(empty.depth.iter().all(|depth| *depth == 1.0));
}

#[test]
fn shared_preparation_matches_cpu_and_updates_same_count_lod_and_identity() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    println!("F7A adapter: {:?}", gpu.adapter_info());
    let mut scene = SnapshotScene::procedural_lod_fixture();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.view = render_core::Mat4::IDENTITY;
    snapshot.camera_position = Vec3::ZERO;
    snapshot.projection =
        render_core::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 321.0 / 181.0, 1.0, 1000.0);
    snapshot.nodes[0].transform.translation = Vec3::new(18.0, 0.0, -10.0);
    snapshot.nodes[1].transform.translation = Vec3::new(0.0, 0.0, -10.0);

    scene.update(&snapshot).unwrap();
    let target = OffscreenTarget::new(gpu.clone(), 321, 181).unwrap();
    let config = render_core::CullingConfig {
        width: 321,
        height: 181,
        ..Default::default()
    };
    let prepared = scene.prepare(config);
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared).unwrap();
    let mut coarse_scene = SnapshotScene::default();
    coarse_scene.update(&snapshot).unwrap();
    let coarse = coarse_scene.prepare(config);
    let high_bytes = renderer.resources().index_bytes;
    assert!(renderer.sync_prepared(&coarse).unwrap());
    assert!(renderer.resources().index_bytes < high_bytes);
    assert!(renderer.sync_prepared(&prepared).unwrap());
    assert!(!renderer.sync_prepared(&prepared).unwrap());

    for frame in 0..4 {
        if frame == 1 {
            snapshot.nodes[0].transform.translation = Vec3::new(0.0, 0.0, -300.0);
        }
        if frame == 2 {
            snapshot.nodes[0].entity = render_core::SnapshotId::Destination(77);
        }
        if frame == 3 {
            snapshot.nodes[0].transform.translation.x = 10000.0;
        }
        scene.update(&snapshot).unwrap();
        let prepared = scene.prepare(config);
        if frame == 0 {
            assert!(prepared
                .draws()
                .any(|d| d.visibility == render_core::VisibilityClass::Partial));
        }
        if frame == 1 {
            assert!(prepared
                .draws()
                .any(|d| d.lod == render_core::LodLevel::Low));
        }

        target
            .render_prepared(&mut renderer, &prepared, scene.view_projection())
            .unwrap();
        assert_eq!(renderer.resources().draw_count, prepared.visible);
        let image = target.readback().unwrap();
        let mut reference_renderer = GpuRenderer::from_snapshot(gpu.clone(), &scene).unwrap();
        target
            .render_snapshot(&mut reference_renderer, &scene)
            .unwrap();
        let reference = target.readback().unwrap();
        assert!(image
            .rgba
            .iter()
            .zip(&reference.rgba)
            .all(|(a, b)| a.abs_diff(*b) <= 1));
        assert!(image
            .depth
            .iter()
            .zip(&reference.depth)
            .all(|(a, b)| (a - b).abs() < 0.00001));
        let mut cpu = render_cpu::Framebuffer::new(321, 181).unwrap();
        render_cpu::render_prepared_scene(
            &mut cpu,
            &prepared,
            scene.view_projection(),
            render_cpu::RasterMode::Secuencial,
        )
        .unwrap();
        let mut differing = 0;
        for (index, (a, b)) in cpu
            .color_bytes()
            .chunks_exact(4)
            .zip(image.rgba.chunks_exact(4))
            .enumerate()
        {
            if a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 1) {
                differing += 1;
            }
            let x = index as u32 % 321;
            let y = index as u32 / 321;
            if a != [8, 10, 18, 255] && b != render_gpu::CLEAR_RGBA {
                assert!((cpu.depth_at(x, y).unwrap() - image.depth[index]).abs() < 0.0001);
            }
        }
        assert!(differing < 60, "{differing} differing pixels");
    }
}
