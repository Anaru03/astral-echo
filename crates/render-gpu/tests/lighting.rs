use render_core::{material_fixture, LightingMode, Mat4, MaterialInfo, Vec3};
use render_cpu::{Framebuffer, RasterMode};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};
use std::sync::Arc;

#[test]
fn lighting_modes_preserve_depth_cache_instances_and_basic_cpu_gpu_equivalence() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let original = material_fixture();
    let mut draws: Vec<_> = original.draws().cloned().collect();
    draws[0].node.transform.scale = Vec3::new(-0.8, 1.3, 0.4);
    draws[1].node.transform.scale = Vec3::new(0.7, 1.1, 1.6);
    draws[2].node.transform.scale = Vec3::new(1.0, 1.0, 0.0);
    let mut scene = render_core::PreparedScene::from_draws(draws);
    scene.lighting = original.lighting.clone();
    for draw in original.draws() {
        scene.set_material(draw.node.material, original.material(draw.node.material));
    }

    let vp = Mat4::perspective_rh(1.0, 320.0 / 180.0, 0.1, 100.0);
    scene.lighting.mode = LightingMode::Unlit;
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &scene).unwrap();
    let target = OffscreenTarget::new(gpu.clone(), 320, 180).unwrap();
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let reference = target.readback().unwrap();
    let unlit_stats = renderer.resources();
    let mut lit_colors = Vec::new();
    for mode in [
        LightingMode::Lambert,
        LightingMode::BlinnPhong,
        LightingMode::Pbr,
        LightingMode::Unlit,
    ] {
        scene.lighting.mode = mode;
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        let image = target.readback().unwrap();
        assert_eq!(image.depth, reference.depth);
        assert_eq!(
            renderer.resources().geometry_uploads,
            unlit_stats.geometry_uploads
        );
        assert_eq!(renderer.resources().draw_count, 3);
        if mode == LightingMode::Unlit {
            assert_eq!(image.rgba, reference.rgba);
        } else {
            assert_ne!(image.rgba, reference.rgba);
            image
                .write_png(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join(format!("../../target/f8a-validation/{mode:?}.png")),
                )
                .unwrap();
            lit_colors.push(image.rgba.clone());
        }
        if mode != LightingMode::Pbr {
            let mut cpu = Framebuffer::new(320, 180).unwrap();
            render_cpu::render_prepared_scene(&mut cpu, &scene, vp, RasterMode::Secuencial)
                .unwrap();
            let mut outliers = 0;
            let mut mean = 0u64;
            for (a, b) in cpu
                .color_bytes()
                .chunks_exact(4)
                .zip(image.rgba.chunks_exact(4))
            {
                if a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 2) {
                    outliers += 1;
                }
                mean += a
                    .iter()
                    .zip(b)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)))
                    .sum::<u64>();
            }
            println!(
                "{mode:?}: outliers={outliers}, mean={}",
                mean as f64 / (320.0 * 180.0 * 4.0)
            );
            assert!(outliers < 30);
            assert!((mean as f64) / (320.0 * 180.0 * 4.0) < 0.25);
        }
    }
    assert_ne!(lit_colors[0], lit_colors[1]);
    assert_ne!(lit_colors[1], lit_colors[2]);
    scene.lighting.mode = LightingMode::BlinnPhong;
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let warmed = renderer.resources();
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(
        renderer.resources().dynamic_bytes_written,
        warmed.dynamic_bytes_written
    );
    scene.set_material(
        render_core::MaterialHandle::new(200),
        MaterialInfo::fixture_rough(),
    );
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(
        renderer.resources().geometry_uploads,
        warmed.geometry_uploads
    );
    assert_eq!(renderer.resources().buffer_rebuilds, warmed.buffer_rebuilds);
    assert!(renderer.resources().dynamic_bytes_written > warmed.dynamic_bytes_written);
    assert_eq!(
        renderer.resources().dynamic_bytes_written - warmed.dynamic_bytes_written,
        208
    );
    let before_light = renderer.resources();
    scene.lighting.ambient *= 0.5;
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(
        renderer.resources().dynamic_bytes_written - before_light.dynamic_bytes_written,
        288
    );
    assert_eq!(
        renderer.resources().buffer_rebuilds,
        before_light.buffer_rebuilds
    );
    renderer
        .set_transform(
            0,
            render_core::Transform::from_translation(Vec3::new(-1.0, 0.0, -5.0)),
        )
        .unwrap();
    target.render_matrix(&mut renderer, vp).unwrap();
    let mut pair: Vec<_> = scene.draws().take(1).cloned().collect();
    let mut next = pair[0].clone();
    next.node.entity = render_core::SnapshotId::Destination(100);
    next.node.transform.translation.x += 1.0;
    pair.push(next);
    let mut pair = render_core::PreparedScene::from_draws(pair);
    pair.lighting = scene.lighting.clone();
    pair.set_material(
        render_core::MaterialHandle::new(200),
        MaterialInfo::fixture_rough(),
    );
    target.render_prepared(&mut renderer, &pair, vp).unwrap();
    assert_eq!(renderer.resources().draw_count, 1);
    let mut alpha = MaterialInfo::fixture_rough();
    alpha.base_color.w = 0.5;
    pair.set_material(render_core::MaterialHandle::new(200), alpha);
    target.render_prepared(&mut renderer, &pair, vp).unwrap();
    assert_eq!(renderer.resources().draw_count, 2);
    assert_eq!(
        renderer.resources().geometry_uploads,
        before_light.geometry_uploads
    );
}

#[test]
fn ggx_fragment_matches_surface_reference_and_material_response() {
    use render_core::{
        LightingEnvironment, LodLevel, MaterialHandle, MeshHandle, PreparedDraw, PreparedScene,
        RasterMesh, RasterVertex, RenderLight, RenderLightKind, SnapshotNode, Transform, Vec4,
        VisibilityClass,
    };
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let mesh = Arc::new(RasterMesh::new(
        [
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        ]
        .into_iter()
        .map(|p| RasterVertex::new(p, Vec4::ONE).with_normal(Vec3::Z))
        .collect(),
        vec![0, 1, 2, 0, 2, 3],
    ));
    let mut scene = PreparedScene::from_draws(vec![PreparedDraw {
        node: SnapshotNode::destination(
            0,
            Transform::from_translation(Vec3::new(0.0, 0.0, -4.0)),
            MeshHandle::QUAD,
            MaterialHandle::new(99),
            true,
        ),
        mesh,
        lod: LodLevel::High,
        visibility: VisibilityClass::Inside,
        projected_size_px: 32.0,
    }]);
    let mut environment = LightingEnvironment::default();
    environment.mode = LightingMode::Pbr;
    environment.ambient = Vec3::ZERO;
    environment.set_lights([RenderLight {
        color: Vec3::ONE,
        intensity: 0.4,
        kind: RenderLightKind::Directional {
            direction: Vec3::new(-0.4, -0.2, -1.0),
        },
        casts_shadow: false,
    }]);
    scene.lighting = environment;
    let vp = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &scene).unwrap();
    let target = OffscreenTarget::new(gpu, 65, 65).unwrap();
    let mut samples = Vec::new();
    for material in [
        MaterialInfo::fixture_metal(),
        MaterialInfo::fixture_rough(),
        MaterialInfo::fixture_emissive(),
        MaterialInfo::new(Vec4::ONE, 0.0, 1.0),
    ] {
        scene.set_material(MaterialHandle::new(99), material);
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        let image = target.readback().unwrap();
        let pixel = &image.rgba[(32 * 65 + 32) * 4..(32 * 65 + 32) * 4 + 4];
        let expected = render_core::shade_surface(
            &scene.lighting,
            material,
            Vec4::ONE,
            Vec3::new(0.0, 0.0, -4.0),
            Vec3::Z,
        );
        let expected =
            [expected.x, expected.y, expected.z, expected.w].map(|v| (v * 255.0).round() as u8);
        println!("GGX {pixel:?}, reference {expected:?}");
        assert!(pixel.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2));
        samples.push(pixel.to_vec());
        assert!(image.depth.iter().all(|v| v.is_finite()));
    }
    assert_ne!(samples[0], samples[1]);
    assert_ne!(samples[1], samples[2]);
}
