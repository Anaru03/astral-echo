use render_core::{
    normal_matrix, shade_surface, LightingEnvironment, LightingMode, Mat4, MaterialInfo,
    RasterMesh, RasterVertex, RenderLight, RenderLightKind, RenderSnapshot, SnapshotScene, Vec3,
    Vec4,
};

#[test]
fn legacy_and_authored_normals_handle_rotation_nonuniform_negative_and_singular_scale() {
    let mesh = RasterMesh::triangle(
        RasterVertex::new(Vec3::ZERO, Vec4::ONE),
        RasterVertex::new(Vec3::X, Vec4::ONE),
        RasterVertex::new(Vec3::Y, Vec4::ONE),
    );
    assert!(mesh.resolved_normals().iter().all(|n| *n == Vec3::Z));
    let cube = RasterMesh::cube(Vec3::ZERO, 1.0, Vec4::ONE);
    assert_eq!(cube.resolved_normals()[0], Vec3::Z);
    let model = Mat4::from_rotation_y(0.7) * Mat4::from_scale(Vec3::new(-2.0, 0.3, 4.0));
    let n = normal_matrix(model)
        .transform_vector3(Vec3::new(1.0, 1.0, 0.0).normalize())
        .normalize();
    let tangent = model.transform_vector3(Vec3::new(1.0, -1.0, 0.0));
    assert!(n.dot(tangent).abs() < 0.00001);
    assert_eq!(
        normal_matrix(Mat4::from_scale(Vec3::new(0.0, 1.0, 1.0))),
        Mat4::ZERO
    );
    assert_eq!(RasterMesh::default().resolved_normals(), Vec::<Vec3>::new());
}

#[test]
fn material_sanitization_and_unlit_preserve_legacy_colors() {
    let invalid = MaterialInfo {
        base_color: Vec4::splat(f32::NAN),
        emissive: Vec3::splat(f32::INFINITY),
        roughness: f32::NAN,
        metallic: 2.0,
    }
    .sanitized();
    assert_eq!(invalid.albedo(), Vec4::ONE);
    assert_eq!(invalid.emissive, Vec3::ZERO);
    assert_eq!(invalid.roughness, 1.0);
    assert_eq!(invalid.metallic, 1.0);
    let color = Vec4::new(0.1, 0.2, 0.3, 0.4);
    assert_eq!(
        shade_surface(
            &LightingEnvironment::default(),
            MaterialInfo::fixture_emissive(),
            color,
            Vec3::ZERO,
            Vec3::ZERO
        ),
        color
    );
}

#[test]
fn lambert_emission_point_range_and_light_limits_are_deterministic() {
    let mut environment = LightingEnvironment::default();
    environment.mode = LightingMode::Lambert;
    environment.ambient = Vec3::ZERO;
    let sun = RenderLight {
        color: Vec3::ONE,
        intensity: 0.5,
        kind: RenderLightKind::Directional {
            direction: Vec3::NEG_Z,
        },
        casts_shadow: false,
    };
    environment.set_lights([sun]);
    let color = shade_surface(
        &environment,
        MaterialInfo::default(),
        Vec4::ONE,
        Vec3::ZERO,
        Vec3::Z,
    );
    assert_eq!(color, Vec4::new(0.5, 0.5, 0.5, 1.0));
    assert_eq!(
        shade_surface(
            &environment,
            MaterialInfo::default(),
            Vec4::ONE,
            Vec3::ZERO,
            Vec3::NEG_Z
        ),
        Vec4::new(0.0, 0.0, 0.0, 1.0)
    );
    let point = RenderLight {
        kind: RenderLightKind::Point {
            position: Vec3::Z,
            range: 2.0,
        },
        ..sun
    };
    environment.set_lights([point]);
    let near = shade_surface(
        &environment,
        MaterialInfo::default(),
        Vec4::ONE,
        Vec3::ZERO,
        Vec3::Z,
    );
    assert!((near.x - 0.140625).abs() < 1e-6);
    assert_eq!(
        shade_surface(
            &environment,
            MaterialInfo::default(),
            Vec4::ONE,
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::Z
        )
        .truncate(),
        Vec3::ZERO
    );
    environment.set_lights([sun, sun, point, point, point, point, point]);
    assert_eq!(environment.lights().len(), 5);
    environment.set_lights([]);
    assert_eq!(
        shade_surface(
            &environment,
            MaterialInfo::fixture_emissive(),
            Vec4::ONE,
            Vec3::ZERO,
            Vec3::Z
        )
        .truncate(),
        Vec3::new(0.4, 0.1, 0.02)
    );
}

#[test]
fn snapshot_lights_material_updates_do_not_change_visibility_lod_or_mesh_identity() {
    let snapshot = RenderSnapshot::minimal_demo();
    let mut scene = SnapshotScene::default();
    scene.update(&snapshot).unwrap();
    let unlit = scene.prepare(Default::default());
    scene.set_lighting_mode(LightingMode::BlinnPhong);
    scene.set_material(snapshot.nodes[0].material, MaterialInfo::fixture_metal());
    let lit = scene.prepare(Default::default());
    assert_eq!(lit.lighting.lights().len(), snapshot.lights.len());
    assert_eq!(lit.lighting.camera_position, snapshot.camera_position);
    assert_eq!(
        unlit.draws().map(|d| (d.node, d.lod)).collect::<Vec<_>>(),
        lit.draws().map(|d| (d.node, d.lod)).collect::<Vec<_>>()
    );
    assert!(unlit
        .draws()
        .zip(lit.draws())
        .all(|(a, b)| std::sync::Arc::ptr_eq(&a.mesh, &b.mesh)));
    assert_eq!(scene.snapshot(), &snapshot);
}

#[test]
fn tiny_and_large_normal_transforms_remain_normalized() {
    for scale in [0.00001, 10000000.0] {
        let n = render_core::transform_normal(Mat4::from_scale(Vec3::splat(scale)), Vec3::Z);
        assert!((n - Vec3::Z).length() < 1e-6);
    }
}

#[test]
fn ggx_metallic_and_roughness_change_response_with_constant_albedo() {
    let mut environment = LightingEnvironment::default();
    environment.mode = LightingMode::Pbr;
    environment.ambient = Vec3::ZERO;
    environment.set_lights([RenderLight {
        color: Vec3::ONE,
        intensity: 0.01,
        kind: RenderLightKind::Directional {
            direction: Vec3::NEG_Z,
        },
        casts_shadow: false,
    }]);
    let sample = |roughness, metallic| {
        shade_surface(
            &environment,
            MaterialInfo::new(Vec4::splat(0.5), roughness, metallic),
            Vec4::ONE,
            Vec3::new(0.0, 0.0, -4.0),
            Vec3::Z,
        )
        .x
    };
    assert!(sample(0.15, 0.0) > sample(0.95, 0.0));
    assert!(sample(0.5, 1.0) > sample(0.5, 0.0));
}
