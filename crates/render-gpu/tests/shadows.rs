use render_core::{shadow_fixture, LightingMode, Mat4, PreparedScene, ShadowResolution, Vec3};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};
use std::sync::Arc;

#[test]
fn directional_shadow_occlusion_preserves_color_depth_and_resources() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let mut scene = shadow_fixture();
    let vp = Mat4::perspective_rh(1.0, 320.0 / 240.0, 0.1, 100.0)
        * Mat4::look_at_rh(scene.lighting.camera_position, Vec3::ZERO, Vec3::Y);
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &scene).unwrap();
    let timing = renderer.enable_gpu_timing();
    let mut target = OffscreenTarget::new(gpu.clone(), 320, 240).unwrap();
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let off = target.readback().unwrap();
    assert_eq!(renderer.resources().shadow_map_bytes, 0);
    scene.lighting.shadows.enabled = true;
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let on = target.readback().unwrap();
    assert_eq!(off.depth, on.depth);
    let darkened = off
        .rgba
        .chunks_exact(4)
        .zip(on.rgba.chunks_exact(4))
        .filter(|(a, b)| a[0] > b[0].saturating_add(40))
        .count();
    println!("shadow pixels: {darkened}");
    off.write_png(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/f8b-validation/off.png"),
    )
    .unwrap();
    assert!(darkened > 100);
    let stats = renderer.resources();
    assert_eq!(stats.shadow_map_allocations, 1);
    assert_eq!(stats.shadow_draw_count, 2);
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(renderer.resources().buffer_rebuilds, stats.buffer_rebuilds);
    assert_eq!(
        renderer.resources().dynamic_bytes_written,
        stats.dynamic_bytes_written
    );
    on.write_png(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/f8b-validation/shadows.png"),
    )
    .unwrap();
    if timing {
        let times = renderer.read_gpu_timings().unwrap().unwrap();
        assert!(times.shadow_ms > 0.0 && times.color_ms > 0.0);
    }
    for resolution in [
        ShadowResolution::R512,
        ShadowResolution::R1024,
        ShadowResolution::R2048,
    ] {
        scene.lighting.shadows.resolution = resolution;
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        assert_eq!(
            renderer.resources().shadow_map_bytes,
            u64::from(resolution as u32).pow(2) * 4
        );
        let allocations = renderer.resources().shadow_map_allocations;
        scene.lighting.shadows.pcf = true;
        scene.lighting.shadows.depth_bias = 0.0005;
        scene.lighting.shadows.normal_bias = 0.001;
        target.resize(400, 300).unwrap();
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        assert_eq!(renderer.resources().shadow_map_allocations, allocations);
        assert_eq!(
            renderer.resources().geometry_uploads,
            stats.geometry_uploads
        );
        let filtered = target.readback().unwrap();
        scene.lighting.shadows.pcf = false;
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        let hard = target.readback().unwrap();
        assert_eq!(filtered.depth, hard.depth);
        assert_ne!(filtered.rgba, hard.rgba);
    }
    target.resize(320, 240).unwrap();
    scene.lighting.shadows.resolution = ShadowResolution::R1024;
    scene.lighting.shadows.depth_bias = 0.001;
    for mode in [
        LightingMode::Lambert,
        LightingMode::BlinnPhong,
        LightingMode::Pbr,
        LightingMode::Unlit,
    ] {
        scene.lighting.mode = mode;
        scene.lighting.shadows.enabled = false;
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        let unshadowed = target.readback().unwrap();
        scene.lighting.shadows.enabled = true;
        target.render_prepared(&mut renderer, &scene, vp).unwrap();
        let shadowed = target.readback().unwrap();
        assert_eq!(unshadowed.depth, shadowed.depth);
        if mode == LightingMode::Unlit {
            assert_eq!(unshadowed.rgba, shadowed.rgba);
            assert_eq!(renderer.resources().shadow_draw_count, 0);
        } else {
            assert_ne!(unshadowed.rgba, shadowed.rgba);
        }
    }
    scene.lighting.mode = LightingMode::Lambert;
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let before_light = target.readback().unwrap();
    let allocations = renderer.resources().shadow_map_allocations;
    let mut lights = scene.lighting.lights().to_vec();
    lights[0].kind = render_core::RenderLightKind::Directional {
        direction: Vec3::new(-0.7, -1.0, 0.2),
    };
    scene.lighting.set_lights(lights.clone());
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_ne!(target.readback().unwrap().rgba, before_light.rgba);
    assert_eq!(renderer.resources().shadow_map_allocations, allocations);
    lights[0].casts_shadow = false;
    scene.lighting.set_lights(lights.clone());
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(renderer.resources().shadow_draw_count, 0);
    lights[0].casts_shadow = true;
    scene.lighting.set_lights(lights);
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(renderer.resources().shadow_map_allocations, allocations);
    let mut draws: Vec<_> = scene.draws().cloned().collect();
    let environment = scene.lighting.clone();
    draws[1].lod = render_core::LodLevel::Low;
    draws[1].mesh = Arc::new(render_core::RasterMesh::cube(
        Vec3::ZERO,
        0.8,
        render_core::Vec4::ONE,
    ));
    let mut low = PreparedScene::from_draws(draws.clone());
    low.lighting = environment.clone();
    target.render_prepared(&mut renderer, &low, vp).unwrap();
    let cached = renderer.resources();
    let mut hidden = PreparedScene::from_draws(vec![draws[0].clone()]);
    hidden.lighting = environment.clone();
    target.render_prepared(&mut renderer, &hidden, vp).unwrap();
    assert_eq!(renderer.resources().shadow_draw_count, 1);
    target.render_prepared(&mut renderer, &low, vp).unwrap();
    assert_eq!(renderer.resources().shadow_draw_count, 2);
    assert_eq!(
        renderer.resources().geometry_uploads,
        cached.geometry_uploads
    );
    assert_eq!(renderer.resources().buffer_rebuilds, cached.buffer_rebuilds);
    let before_transform = target.readback().unwrap();
    draws[1].node.transform.translation.x = 1.5;
    draws[1].node.transform.scale = Vec3::new(-1.2, 0.7, 0.9);
    renderer.set_transform(1, draws[1].node.transform).unwrap();
    target.render_matrix(&mut renderer, vp).unwrap();
    let direct = target.readback().unwrap();
    assert_ne!(before_transform.rgba, direct.rgba);
    low = PreparedScene::from_draws(draws);
    low.lighting = environment;
    target.render_prepared(&mut renderer, &low, vp).unwrap();
    let synced = target.readback().unwrap();
    assert_eq!(direct.rgba, synced.rgba);
    assert_eq!(direct.depth, synced.depth);
    assert_eq!(renderer.resources().shadow_map_allocations, allocations);
    scene.lighting.shadows.depth_bias = f32::NAN;
    assert!(matches!(
        renderer.sync_prepared(&scene),
        Err(GpuError::InvalidInput(_))
    ));
}

#[test]
fn software_rendering_ignores_shadow_settings_without_gpu() {
    assert!(matches!(
        GpuContext::new(render_gpu::GpuOptions {
            backends: wgpu::Backends::empty(),
            force_fallback_adapter: false
        }),
        Err(GpuError::Unavailable(_))
    ));
    let mut scene = shadow_fixture();
    let vp = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0)
        * Mat4::look_at_rh(scene.lighting.camera_position, Vec3::ZERO, Vec3::Y);
    let mut framebuffer = render_cpu::Framebuffer::new(128, 128).unwrap();
    render_cpu::render_prepared_scene(
        &mut framebuffer,
        &scene,
        vp,
        render_cpu::RasterMode::Secuencial,
    )
    .unwrap();
    let off = framebuffer.color_bytes().to_vec();
    let depth = framebuffer.depth_slice_mut().to_vec();
    scene.lighting.shadows.enabled = true;
    render_cpu::render_prepared_scene(
        &mut framebuffer,
        &scene,
        vp,
        render_cpu::RasterMode::Secuencial,
    )
    .unwrap();
    assert_eq!(off, framebuffer.color_bytes());
    assert_eq!(depth, framebuffer.depth_slice_mut());
}

#[test]
fn bias_controls_acne_and_excessive_bias_detaches_shadows() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let mut scene = shadow_fixture();
    scene.lighting.shadows.enabled = true;
    scene.lighting.shadows.resolution = ShadowResolution::R512;
    let vp = Mat4::perspective_rh(1.0, 320.0 / 240.0, 0.1, 100.0)
        * Mat4::look_at_rh(scene.lighting.camera_position, Vec3::ZERO, Vec3::Y);
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &scene).unwrap();
    let target = OffscreenTarget::new(gpu, 320, 240).unwrap();
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let biased = target.readback().unwrap();
    scene.lighting.shadows.depth_bias = 0.2;
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let detached = target.readback().unwrap();
    let removed = biased
        .rgba
        .chunks_exact(4)
        .zip(detached.rgba.chunks_exact(4))
        .filter(|(a, b)| b[0] > a[0].saturating_add(40))
        .count();
    assert!(
        removed > 100,
        "excessive bias should erase the contact shadow: {removed}"
    );
    let mut plane = PreparedScene::from_draws(vec![scene.draws().next().unwrap().clone()]);
    plane.lighting = scene.lighting.clone();
    plane.lighting.shadows.depth_bias = 0.0;
    plane.lighting.shadows.slope_bias = 0.0;
    target.render_prepared(&mut renderer, &plane, vp).unwrap();
    let acne = target.readback().unwrap();
    plane.lighting.shadows.depth_bias = 0.001;
    plane.lighting.shadows.slope_bias = 0.002;
    target.render_prepared(&mut renderer, &plane, vp).unwrap();
    let clean = target.readback().unwrap();
    let acne_pixels = acne
        .rgba
        .chunks_exact(4)
        .zip(clean.rgba.chunks_exact(4))
        .filter(|(a, b)| b[0] > a[0].saturating_add(40))
        .count();
    println!("acne corrected: {acne_pixels}; detached contact: {removed}");
    assert!(acne_pixels > 100);
    plane.lighting.shadows.enabled = false;
    target.render_prepared(&mut renderer, &plane, vp).unwrap();
    assert_eq!(target.readback().unwrap().rgba, clean.rgba);
}
