use render_gpu::{aces_tonemap, PostProcessConfig, PostProcessPreset};

#[test]
fn presets_and_exposure_are_finite_and_monotonic() {
    assert!(!PostProcessConfig::preset(PostProcessPreset::Low).enabled);
    assert!(PostProcessConfig::preset(PostProcessPreset::Medium).bloom);
    assert!(PostProcessConfig::preset(PostProcessPreset::High).fxaa);
    assert!(PostProcessConfig::default().validate().is_ok());
    assert!(PostProcessConfig {
        exposure: 0.0,
        ..Default::default()
    }
    .validate()
    .is_err());
    let low = aces_tonemap([1.0, 0.5, 0.1], 0.5);
    let high = aces_tonemap([1.0, 0.5, 0.1], 2.0);
    assert!(high.iter().zip(low).all(|(a, b)| *a >= b));
    assert!(high
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
}

#[test]
fn disabled_postprocess_preserves_existing_target_and_enabled_allocates_persistently() {
    let gpu = match render_gpu::GpuContext::new(Default::default()) {
        Ok(gpu) => std::sync::Arc::new(gpu),
        Err(render_gpu::GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let scene = render_core::shadow_fixture();
    let vp = render_core::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
    let mut renderer = render_gpu::GpuRenderer::from_prepared(gpu.clone(), &scene).unwrap();
    let mut target = render_gpu::OffscreenTarget::new(gpu, 96, 64).unwrap();
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let ldr = target.readback().unwrap();
    target
        .set_postprocess(PostProcessConfig {
            enabled: true,
            exposure: 1.5,
            bloom: true,
            fxaa: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(target.postprocess_resources().unwrap().2, 1);
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let processed = target.readback().unwrap();
    assert_eq!(ldr.depth, processed.depth);
    assert_ne!(ldr.rgba, processed.rgba);
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    assert_eq!(target.postprocess_resources().unwrap().2, 1);
    target.resize(128, 72).unwrap();
    assert_eq!(target.postprocess_resources().unwrap().2, 1);
    assert!(target.postprocess_config().unwrap().fxaa);
}

#[test]
fn hdr_renderer_accepts_rgba16float_and_tone_maps_emissive_values_above_one() {
    let gpu = match render_gpu::GpuContext::new(Default::default()) {
        Ok(gpu) => std::sync::Arc::new(gpu),
        Err(render_gpu::GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let mut scene = render_core::shadow_fixture();
    scene.lighting.mode = render_core::LightingMode::Lambert;
    let draw = scene.draws().next().unwrap().node;
    scene.set_material(
        draw.material,
        render_core::MaterialInfo::new(render_core::Vec4::ONE, 0.4, 0.0)
            .with_emissive(render_core::Vec3::splat(8.0)),
    );
    let vp = render_core::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
    let mut renderer = render_gpu::GpuRenderer::from_prepared_hdr(gpu.clone(), &scene).unwrap();
    let mut target = render_gpu::OffscreenTarget::new_hdr(gpu, 64, 64).unwrap();
    target
        .set_postprocess(PostProcessConfig {
            enabled: true,
            exposure: 0.5,
            ..Default::default()
        })
        .unwrap();
    target.render_prepared(&mut renderer, &scene, vp).unwrap();
    let image = target.readback().unwrap();
    assert_eq!(renderer.color_format(), wgpu::TextureFormat::Rgba16Float);
    assert!(image.rgba.chunks_exact(4).any(|pixel| pixel[0] > 180));
}
