use std::sync::Arc;

use render_core::{
    CullingConfig, LodLevel, Mat4, MaterialHandle, MeshHandle, PreparedDraw, PreparedScene,
    RasterMesh, RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3, Vec4,
    VisibilityClass,
};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};

fn gpu() -> Option<Arc<GpuContext>> {
    match GpuContext::new(Default::default()) {
        Ok(gpu) => Some(Arc::new(gpu)),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            None
        }
        Err(error) => panic!("{error}"),
    }
}

fn draw(id: u32, mesh: Arc<RasterMesh>, material: u32) -> PreparedDraw {
    PreparedDraw {
        node: SnapshotNode::destination(
            id,
            Transform::IDENTITY,
            MeshHandle::CUBE,
            MaterialHandle::new(material),
            true,
        ),
        mesh,
        lod: LodLevel::High,
        visibility: VisibilityClass::Inside,
        projected_size_px: 100.0,
    }
}

fn compare(
    gpu: Arc<GpuContext>,
    renderer: &mut GpuRenderer,
    target: &OffscreenTarget,
    prepared: &PreparedScene,
    vp: Mat4,
) {
    target.render_prepared(renderer, prepared, vp).unwrap();
    let image = target.readback().unwrap();
    let mut reference = GpuRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm, &[]).unwrap();
    reference.sync_prepared_reference(prepared).unwrap();
    target.render_matrix(&mut reference, vp).unwrap();
    let expected = target.readback().unwrap();
    assert_eq!(image.rgba, expected.rgba);
    assert_eq!(image.depth, expected.depth);
    let mut cpu = render_cpu::Framebuffer::new(160, 90).unwrap();
    render_cpu::render_prepared_scene(&mut cpu, prepared, vp, render_cpu::RasterMode::Secuencial)
        .unwrap();
    let mut outliers = 0;
    for (i, (a, b)) in cpu
        .color_bytes()
        .chunks_exact(4)
        .zip(image.rgba.chunks_exact(4))
        .enumerate()
    {
        if a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 1) {
            outliers += 1;
        }
        if a != [8, 10, 18, 255] && b != render_gpu::CLEAR_RGBA {
            assert!(
                (cpu.depth_at(i as u32 % 160, i as u32 / 160).unwrap() - image.depth[i]).abs()
                    < 0.0001
            );
        }
    }
    assert!(outliers < 30, "{outliers} CPU/GPU outliers");
}

#[test]
fn geometry_and_instance_buffers_survive_visibility_lod_and_transform_changes() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut snapshot = RenderSnapshot::minimal_demo();
    let template = snapshot.nodes[0];
    snapshot.nodes.clear();
    snapshot.view = Mat4::IDENTITY;
    snapshot.camera_position = Vec3::ZERO;
    snapshot.projection = Mat4::perspective_rh(1.0, 160.0 / 90.0, 0.1, 1000.0);
    for id in 0..1000 {
        let mut node = template;
        node.entity = render_core::SnapshotId::Destination(id);
        node.transform.translation =
            Vec3::new((id % 40) as f32 - 19.5, (id / 40) as f32 - 12.0, -80.0);
        node.transform.scale = Vec3::splat(0.1);
        snapshot.nodes.push(node);
    }
    let mut scene = SnapshotScene::procedural_lod_fixture();
    scene.update(&snapshot).unwrap();
    let config = CullingConfig {
        width: 160,
        height: 90,
        low_threshold_px: 0.0,
        high_threshold_px: 0.0,
    };
    let prepared = scene.prepare(config);
    assert_eq!(prepared.visible, 1000);
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared).unwrap();
    let target = OffscreenTarget::new(gpu.clone(), 160, 90).unwrap();
    compare(
        gpu.clone(),
        &mut renderer,
        &target,
        &prepared,
        scene.view_projection(),
    );
    let first = renderer.resources();
    assert_eq!(first.draw_count, 1);
    assert_eq!(first.instance_count, 1000);
    assert_eq!(first.geometry_uploads, 1);
    target
        .render_prepared(&mut renderer, &prepared, scene.view_projection())
        .unwrap();
    assert_eq!(
        renderer.resources().dynamic_bytes_written,
        first.dynamic_bytes_written
    );
    snapshot.nodes[500].transform.translation.x += 0.5;
    scene.update(&snapshot).unwrap();
    let moved = scene.prepare(config);
    target
        .render_prepared(&mut renderer, &moved, scene.view_projection())
        .unwrap();
    assert_eq!(
        renderer.resources().dynamic_bytes_written - first.dynamic_bytes_written,
        64
    );
    let before_camera = renderer.resources();
    target
        .render_matrix(
            &mut renderer,
            scene.view_projection() * Mat4::from_translation(Vec3::new(0.1, 0.0, 0.0)),
        )
        .unwrap();
    assert_eq!(
        renderer.resources().dynamic_bytes_written - before_camera.dynamic_bytes_written,
        64000
    );
    assert_eq!(
        renderer.resources().buffer_rebuilds,
        before_camera.buffer_rebuilds
    );
    let allocations = renderer.resources().buffer_rebuilds;
    let nodes = snapshot.nodes.clone();
    snapshot.nodes.clear();
    scene.update(&snapshot).unwrap();
    let empty = scene.prepare(config);
    compare(
        gpu.clone(),
        &mut renderer,
        &target,
        &empty,
        scene.view_projection(),
    );
    assert_eq!(renderer.resources().draw_count, 0);
    assert_eq!(renderer.resources().instance_bytes, first.instance_bytes);
    snapshot.nodes = nodes;
    for node in snapshot.nodes.iter_mut().step_by(2) {
        node.visible = false;
    }
    scene.update(&snapshot).unwrap();
    let half = scene.prepare(config);
    compare(
        gpu.clone(),
        &mut renderer,
        &target,
        &half,
        scene.view_projection(),
    );
    assert_eq!(renderer.resources().instance_count, 500);
    assert_eq!(renderer.resources().geometry_uploads, 1);
    assert_eq!(renderer.resources().buffer_rebuilds, allocations);
    let low = scene.prepare(CullingConfig {
        low_threshold_px: 10000.0,
        high_threshold_px: 20000.0,
        ..config
    });
    assert!(low.draws().all(|draw| draw.lod == LodLevel::Low));
    compare(
        gpu.clone(),
        &mut renderer,
        &target,
        &low,
        scene.view_projection(),
    );
    assert_eq!(renderer.resources().geometry_uploads, 2);
    let mixed = PreparedScene::from_draws(
        half.draws()
            .take(250)
            .chain(low.draws().skip(250))
            .cloned()
            .collect(),
    );
    compare(
        gpu.clone(),
        &mut renderer,
        &target,
        &mixed,
        scene.view_projection(),
    );
    assert_eq!(renderer.resources().draw_count, 2);
    let mut grown: Vec<_> = prepared.draws().cloned().collect();
    for id in 1000..1500 {
        let mut next = grown[0].clone();
        next.node.entity = render_core::SnapshotId::Destination(id);
        grown.push(next);
    }
    let grown = PreparedScene::from_draws(grown);
    let before_growth = renderer.resources();
    compare(
        gpu.clone(),
        &mut renderer,
        &target,
        &grown,
        scene.view_projection(),
    );
    assert_eq!(
        renderer.resources().buffer_rebuilds,
        before_growth.buffer_rebuilds + 1
    );
    assert_eq!(
        renderer.resources().geometry_uploads,
        before_growth.geometry_uploads
    );
    assert_eq!(
        renderer.resources().instance_bytes,
        first.instance_bytes * 2
    );
    let warmed = renderer.resources();
    for prepared in [&half, &low, &empty, &half, &low] {
        compare(
            gpu.clone(),
            &mut renderer,
            &target,
            prepared,
            scene.view_projection(),
        );
        let stats = renderer.resources();
        assert_eq!(stats.geometry_uploads, warmed.geometry_uploads);
        assert_eq!(stats.buffer_rebuilds, warmed.buffer_rebuilds);
        assert_eq!(stats.vertex_bytes, warmed.vertex_bytes);
        assert_eq!(stats.index_bytes, warmed.index_bytes);
        assert_eq!(stats.instance_bytes, warmed.instance_bytes);
    }
}

#[test]
fn batches_preserve_material_boundaries_coplanar_winners_alpha_and_mesh_versions() {
    let Some(gpu) = gpu() else {
        return;
    };
    let red = Arc::new(RasterMesh::cube(
        Vec3::ZERO,
        1.0,
        Vec4::new(1.0, 0.0, 0.0, 1.0),
    ));
    let green = Arc::new(RasterMesh::cube(
        Vec3::ZERO,
        1.0,
        Vec4::new(0.0, 1.0, 0.0, 1.0),
    ));
    let alpha = Arc::new(RasterMesh::cube(
        Vec3::ZERO,
        1.0,
        Vec4::new(0.0, 0.0, 1.0, 0.5),
    ));
    let prepared = PreparedScene::from_draws(vec![
        draw(0, red.clone(), 0),
        draw(1, red.clone(), 0),
        draw(2, green, 1),
        draw(3, red.clone(), 0),
        draw(4, red.clone(), 2),
        draw(5, alpha.clone(), 0),
        draw(6, alpha, 0),
    ]);
    let vp = Mat4::perspective_rh(1.0, 160.0 / 90.0, 0.1, 100.0)
        * Mat4::from_translation(Vec3::new(0.0, 0.0, -4.0));
    let target = OffscreenTarget::new(gpu.clone(), 160, 90).unwrap();
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared).unwrap();
    assert_eq!(renderer.resources().draw_count, 6);
    assert_eq!(renderer.resources().instance_count, 7);
    assert_eq!(renderer.resources().geometry_uploads, 3);
    let mut same_geometry_lod: Vec<_> = prepared.draws().cloned().collect();
    for draw in &mut same_geometry_lod {
        draw.lod = LodLevel::Low;
    }
    let same_geometry_lod = PreparedScene::from_draws(same_geometry_lod);
    compare(gpu.clone(), &mut renderer, &target, &same_geometry_lod, vp);
    assert_eq!(renderer.resources().geometry_uploads, 3);
    compare(gpu.clone(), &mut renderer, &target, &prepared, vp);
    target.render_matrix(&mut renderer, vp).unwrap();
    let first = target.readback().unwrap();
    assert_eq!(
        &first.rgba[(45 * 160 + 80) * 4..(45 * 160 + 80) * 4 + 4],
        &[255, 0, 0, 255]
    );
    let mut reversed: Vec<_> = prepared.draws().cloned().collect();
    reversed.reverse();
    let reversed = PreparedScene::from_draws(reversed);
    compare(gpu, &mut renderer, &target, &reversed, vp);
    target.render_matrix(&mut renderer, vp).unwrap();
    let reversed_image = target.readback().unwrap();
    assert_eq!(
        &reversed_image.rgba[(45 * 160 + 80) * 4..(45 * 160 + 80) * 4 + 3],
        &[0, 0, 255]
    );
    assert!(reversed_image.rgba[(45 * 160 + 80) * 4 + 3].abs_diff(128) <= 1);
    assert_eq!(renderer.resources().geometry_uploads, 3);
}
