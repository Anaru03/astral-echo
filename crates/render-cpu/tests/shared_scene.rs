use std::any::TypeId;

#[test]
fn colored_mesh_types_are_shared_with_render_core() {
    assert_eq!(
        TypeId::of::<render_cpu::RasterMesh>(),
        TypeId::of::<render_core::RasterMesh>()
    );
    assert_eq!(
        TypeId::of::<render_cpu::RasterVertex>(),
        TypeId::of::<render_core::RasterVertex>()
    );
    let scene = render_core::demo_scene();
    assert_eq!(scene.len(), 19);
    assert_eq!(
        scene
            .iter()
            .map(|(mesh, _)| mesh.indices.len() / 3)
            .sum::<usize>(),
        278
    );
    assert_eq!(scene, render_cpu::demo_scene());
}

#[test]
fn cpu_camera_wrapper_preserves_shared_camera_and_error_contract() {
    for aspect in [1.0, 16.0 / 9.0, 2.0] {
        assert_eq!(
            render_cpu::demo_camera(aspect).unwrap(),
            render_core::demo_camera(aspect).unwrap()
        );
    }
    assert_eq!(
        render_cpu::demo_camera(0.0),
        Err(render_cpu::RasterError::InvalidViewport)
    );
    assert!(render_core::demo_camera(0.0).is_err());
}

#[test]
fn shared_assembly_matches_reference_and_invalidates_on_mesh_topology_and_plain_draws() {
    use render_core::{Mat4, RasterMesh, Vec3, Vec4};
    use render_cpu::{Color, CpuPipeline, Framebuffer, PipelineConfig, Topology, Viewport};
    use std::sync::Arc;
    let cube = Arc::new(RasterMesh::cube(
        Vec3::ZERO,
        1.0,
        Vec4::new(1.0, 0.0, 0.0, 1.0),
    ));
    let other = Arc::new(RasterMesh::cube(
        Vec3::ZERO,
        0.5,
        Vec4::new(0.0, 1.0, 0.0, 0.5),
    ));
    let vp = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0)
        * Mat4::from_translation(Vec3::new(0.0, 0.0, -4.0));
    let mut reference = Framebuffer::new(64, 64).unwrap();
    let mut optimized = Framebuffer::new(64, 64).unwrap();
    let mut hits = 0;
    let mut metrics = None;
    for (framebuffer, shared) in [(&mut reference, false), (&mut optimized, true)] {
        let mut pipeline = CpuPipeline::new(
            framebuffer,
            Viewport::from_size(64, 64).unwrap(),
            PipelineConfig::default(),
        );
        pipeline.clear(Color::BLACK);
        for (mesh, topology, count, plain) in [
            (&cube, Topology::TriangleList, 2, false),
            (&other, Topology::TriangleList, 1, false),
            (&cube, Topology::TriangleList, 1, false),
            (&cube, Topology::TriangleFan, 2, false),
            (&other, Topology::TriangleList, 1, true),
            (&cube, Topology::TriangleList, 2, false),
        ] {
            if shared && !plain {
                pipeline
                    .draw_mesh_instances(mesh, std::iter::repeat_n(vp, count), topology)
                    .unwrap();
            } else {
                for _ in 0..count {
                    pipeline.draw_mesh(mesh, vp, topology).unwrap();
                }
            }
        }
        if shared {
            hits = pipeline.stats.assembly_cache_hits;
            assert_eq!(Some(pipeline.stats.metrics), metrics);
        } else {
            metrics = Some(pipeline.stats.metrics);
        }
    }
    assert_eq!(hits, 3);
    assert_eq!(reference, optimized);
    assert_eq!(Arc::strong_count(&cube), 1);
    assert_eq!(Arc::strong_count(&other), 1);
}

#[test]
fn lit_normals_materials_and_depth_work_without_gpu() {
    use render_core::{LightingMode, Mat4, Vec3};
    use render_cpu::{Framebuffer, RasterMode};
    let mut scene = render_core::material_fixture();
    let mut draws: Vec<_> = scene.draws().cloned().collect();
    draws[0].node.transform.scale = Vec3::new(-1.2, 0.4, 2.0);
    draws[0].node.transform.rotation = render_core::Quat::from_rotation_y(0.5);
    let mut changed = render_core::PreparedScene::from_draws(draws);
    changed.lighting = scene.lighting.clone();
    for draw in scene.draws() {
        changed.set_material(draw.node.material, scene.material(draw.node.material));
    }
    scene = changed;
    let vp = Mat4::perspective_rh(1.0, 160.0 / 90.0, 0.1, 100.0);
    let mut unlit = Framebuffer::new(160, 90).unwrap();
    scene.lighting.mode = LightingMode::Unlit;
    render_cpu::render_prepared_scene(&mut unlit, &scene, vp, RasterMode::Secuencial).unwrap();
    for mode in [
        LightingMode::Lambert,
        LightingMode::BlinnPhong,
        LightingMode::Pbr,
    ] {
        scene.lighting.mode = mode;
        let mut lit = Framebuffer::new(160, 90).unwrap();
        render_cpu::render_prepared_scene(&mut lit, &scene, vp, RasterMode::Secuencial).unwrap();
        assert_ne!(lit.color_bytes(), unlit.color_bytes());
        assert_eq!(lit.depth_slice_mut(), unlit.depth_slice_mut());
        for raster in [RasterMode::Paralelo, RasterMode::Auto] {
            let mut other = Framebuffer::new(160, 90).unwrap();
            render_cpu::render_prepared_scene(&mut other, &scene, vp, raster).unwrap();
            assert_eq!(lit, other);
        }
    }
}
