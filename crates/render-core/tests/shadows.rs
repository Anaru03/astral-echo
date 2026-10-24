use render_core::{shadow_fixture, LightingMode, Mat4, ShadowConfig, Vec3};

#[test]
fn shadow_configuration_is_opt_in_and_validated() {
    assert!(!ShadowConfig::default().enabled);
    assert!(!ShadowConfig::low().enabled);
    assert_eq!(ShadowConfig::low().resolution as u32, 512);
    let mut config = ShadowConfig::default();
    for value in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        config.half_extent = value;
        assert!(config.validate().is_err());
    }
    for field in 0..4 {
        for value in [-1.0, f32::NAN, f32::INFINITY] {
            let mut config = ShadowConfig::default();
            match field {
                0 => config.depth_bias = value,
                1 => config.slope_bias = value,
                2 => config.normal_bias = value,
                _ => config.depth = value,
            }
            assert!(config.validate().is_err());
        }
    }
    assert!(ShadowConfig::default().view_projection(Vec3::ZERO).is_err());
    assert!(ShadowConfig::default()
        .view_projection(Vec3::splat(f32::NAN))
        .is_err());
}

#[test]
fn light_projection_uses_wgpu_depth_and_handles_vertical_light() {
    let config = ShadowConfig::default();
    for direction in [Vec3::NEG_Y, Vec3::Y, Vec3::new(-0.5, -1.0, 0.2).normalize()] {
        let vp = config.view_projection(direction).unwrap();
        assert!(vp.is_finite());
        let eye = config.center - direction * config.depth * 0.5;
        assert!(vp.project_point3(eye + direction * 0.1).z.abs() < 0.00001);
        assert!((vp.project_point3(eye + direction * config.depth).z - 1.0).abs() < 0.00001);
        assert!((vp.project_point3(config.center).z - 0.5).abs() < 0.01);
    }
}

#[test]
fn only_enabled_lit_shadow_casting_directional_light_selects_map() {
    let mut scene = shadow_fixture();
    assert_eq!(scene.lighting.shadow_view_projection(), Ok(None));
    scene.lighting.shadows.enabled = true;
    assert!(scene.lighting.shadow_view_projection().unwrap().is_some());
    scene.lighting.mode = LightingMode::Unlit;
    assert_eq!(scene.lighting.shadow_view_projection(), Ok(None));
    scene.lighting.mode = LightingMode::Lambert;
    let mut lights = scene.lighting.lights().to_vec();
    lights[0].casts_shadow = false;
    scene.lighting.set_lights(lights);
    assert_eq!(scene.lighting.shadow_view_projection(), Ok(None));
    scene.lighting.set_lights([render_core::RenderLight {
        casts_shadow: true,
        ..render_core::RenderLight::demo_point()
    }]);
    assert_eq!(scene.lighting.shadow_view_projection(), Ok(None));
}

#[test]
fn camera_culling_retains_off_camera_casters_and_excludes_hidden() {
    use render_core::{
        CullingConfig, MeshHandle, RenderSnapshot, SnapshotId, SnapshotLight, SnapshotNode,
        SnapshotScene, Transform, VisualAssets,
    };
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.nodes.clear();
    snapshot.lights.clear();
    snapshot.view = Mat4::IDENTITY;
    snapshot.projection = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
    snapshot.camera_position = Vec3::ZERO;
    let fixture = shadow_fixture();
    snapshot.push_light(SnapshotLight::new(fixture.lighting.lights()[0]));
    for (id, position, visible) in [
        (0, Vec3::new(0.0, 0.0, -8.0), true),
        (1, Vec3::new(7.0, 0.0, -2.0), true),
        (2, Vec3::new(3.0, 0.0, -2.0), false),
        (3, Vec3::splat(2000.0), true),
    ] {
        snapshot.push_node(SnapshotNode::destination(
            id,
            Transform::from_translation(position),
            MeshHandle::CUBE,
            VisualAssets::PLAYER,
            visible,
        ));
    }
    let mut scene = SnapshotScene::procedural_lod_fixture();
    scene.set_lighting_mode(LightingMode::Lambert);
    scene.update(&snapshot).unwrap();
    let off = scene.prepare(CullingConfig::default());
    scene.set_shadow_config(ShadowConfig {
        enabled: true,
        half_extent: 12.0,
        ..Default::default()
    });
    let on = scene.prepare(CullingConfig::default());
    assert_eq!(off.visible, on.visible);
    assert_eq!(off.triangles, on.triangles);
    assert_eq!(
        off.draws()
            .map(|d| (d.node.entity, d.lod))
            .collect::<Vec<_>>(),
        on.draws()
            .map(|d| (d.node.entity, d.lod))
            .collect::<Vec<_>>()
    );
    let casters: Vec<_> = on.shadow_draws().map(|d| d.node.entity).collect();
    assert!(casters.contains(&SnapshotId::Destination(1)));
    assert!(!casters.contains(&SnapshotId::Destination(2)));
    assert!(!casters.contains(&SnapshotId::Destination(3)));
    assert_eq!(scene.snapshot().nodes.len(), 4);
}
