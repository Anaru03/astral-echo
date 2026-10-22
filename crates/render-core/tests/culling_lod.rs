use engine::Tick;
use render_core::{
    Camera, CullingConfig, LodLevel, MaterialHandle, MeshHandle, RenderSnapshot, SnapshotId,
    SnapshotNode, SnapshotScene, Transform, Vec3, VisibilityClass, VisualAssets,
};

fn scene_at(position: Vec3, visible: bool) -> SnapshotScene {
    let camera = Camera::demo();
    let mut snapshot = RenderSnapshot::new(
        Tick(1),
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );
    snapshot.push_node(SnapshotNode::new(
        engine::EntityId::new(1),
        Transform::from_translation(position),
        MeshHandle::CUBE,
        VisualAssets::PLAYER,
        visible,
    ));
    let mut scene = SnapshotScene::procedural_lod_fixture();
    scene.update(&snapshot).unwrap();
    scene
}

fn config() -> CullingConfig {
    CullingConfig {
        width: 640,
        height: 360,
        low_threshold_px: 8.0,
        high_threshold_px: 20.0,
    }
}

#[test]
fn culls_outside_and_keeps_partial_aabb() {
    let outside = scene_at(Vec3::new(10_000.0, 0.0, 0.0), true).prepare(config());
    assert_eq!(outside.visible, 0);
    assert_eq!(outside.discarded, 1);

    let mut partial_scene = scene_at(Vec3::new(18.0, 0.0, -18.0), true);
    let mut snapshot = partial_scene.snapshot().clone();
    snapshot.nodes[0].transform.scale = Vec3::splat(100.0);
    partial_scene.update(&snapshot).unwrap();
    let partial = partial_scene.prepare(config());
    assert_eq!(partial.visible, 1);
    assert_eq!(
        partial.draws().next().unwrap().visibility,
        VisibilityClass::Partial
    );
}

#[test]
fn hidden_nodes_are_discarded_without_changing_identity() {
    let prepared = scene_at(Vec3::new(0.0, 0.0, 0.0), false).prepare(config());
    assert_eq!(prepared.considered, 1);
    assert_eq!(prepared.visible, 0);
    assert_eq!(prepared.discarded, 1);
    let scene = scene_at(Vec3::ZERO, false);
    let original = scene.snapshot().clone();
    scene.prepare(config());
    assert_eq!(scene.snapshot(), &original);
    assert_eq!(
        scene.snapshot().nodes[0].entity,
        SnapshotId::Entity(engine::EntityId::new(1))
    );
}

#[test]
fn projected_size_selects_low_and_high_with_hysteresis_gap() {
    let far = scene_at(Vec3::new(0.0, 0.0, -480.0), true).prepare(config());
    assert_eq!(far.draws().next().unwrap().lod, LodLevel::Low);
    let near = scene_at(Vec3::new(0.0, 0.0, 20.0), true).prepare(config());
    assert_eq!(near.draws().next().unwrap().lod, LodLevel::High);
    assert!(near.triangles > far.triangles);
}

#[test]
fn non_uniform_scale_uses_transformed_bounds() {
    let camera = Camera::demo();
    let mut snapshot = RenderSnapshot::new(
        Tick(1),
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );
    let mut transform = Transform::from_translation(Vec3::new(0.0, 0.0, -100.0));
    transform.scale = Vec3::new(20.0, 0.2, 0.2);
    snapshot.push_node(SnapshotNode::new(
        engine::EntityId::new(1),
        transform,
        MeshHandle::CUBE,
        MaterialHandle::new(2),
        true,
    ));
    let mut scene = SnapshotScene::procedural_lod_fixture();
    scene.update(&snapshot).unwrap();
    let prepared = scene.prepare(config());
    assert_eq!(prepared.visible, 1);
    assert!(prepared.draws().next().unwrap().projected_size_px > 8.0);
}

fn frontal_scene(z: f32) -> SnapshotScene {
    let camera = Camera::new(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::Y,
        render_core::Projection::Perspective {
            fov_y_rad: std::f32::consts::FRAC_PI_2,
            aspect: 1.0,
            near: 1.0,
            far: 1000.0,
        },
    )
    .unwrap();
    let mut scene = scene_at(Vec3::new(0.0, 0.0, z), true);
    let mut snapshot = scene.snapshot().clone();
    snapshot.view = camera.view_matrix();
    snapshot.projection = camera.projection_matrix();
    snapshot.camera_position = camera.position();
    scene.update(&snapshot).unwrap();
    scene
}

#[test]
fn hysteresis_survives_updates_and_camera_motion() {
    let mut scene = frontal_scene(-100.0);
    let config = CullingConfig {
        width: 100,
        height: 100,
        low_threshold_px: 8.0,
        high_threshold_px: 20.0,
    };
    assert_eq!(
        scene.prepare(config).draws().next().unwrap().lod,
        LodLevel::Low
    );
    let mut snapshot = scene.snapshot().clone();
    snapshot.view = render_core::Mat4::from_translation(Vec3::new(0.0, 0.0, 90.0));
    snapshot.camera_position.z = -90.0;
    scene.update(&snapshot).unwrap();
    let middle = scene.prepare(config);
    let draw = middle.draws().next().unwrap();
    assert!(draw.projected_size_px > 8.0 && draw.projected_size_px < 20.0);
    assert_eq!(draw.lod, LodLevel::Low);
    snapshot.view = render_core::Mat4::from_translation(Vec3::new(0.0, 0.0, 95.0));
    scene.update(&snapshot).unwrap();
    assert_eq!(
        scene.prepare(config).draws().next().unwrap().lod,
        LodLevel::High
    );
    snapshot.view = render_core::Mat4::from_translation(Vec3::new(0.0, 0.0, 90.0));
    scene.update(&snapshot).unwrap();
    assert_eq!(
        scene.prepare(config).draws().next().unwrap().lod,
        LodLevel::High
    );
    snapshot.nodes.clear();
    scene.update(&snapshot).unwrap();
    snapshot.nodes = frontal_scene(-100.0).snapshot().nodes.clone();
    scene.update(&snapshot).unwrap();
    assert_eq!(
        scene.prepare(config).draws().next().unwrap().lod,
        LodLevel::High
    );
    snapshot.view = render_core::Mat4::IDENTITY;
    scene.update(&snapshot).unwrap();
    assert_eq!(
        scene.prepare(config).draws().next().unwrap().lod,
        LodLevel::Low
    );
}

#[test]
fn six_frustum_planes_keep_touching_and_partial_bounds() {
    let vp = render_core::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 1.0, 10.0);
    let frustum = render_core::Frustum::from_view_projection(vp);
    for point in [
        Vec3::new(-5.0, 0.0, -5.0),
        Vec3::new(5.0, 0.0, -5.0),
        Vec3::new(0.0, -5.0, -5.0),
        Vec3::new(0.0, 5.0, -5.0),
        Vec3::new(0.0, 0.0, -1.0),
        Vec3::new(0.0, 0.0, -10.0),
    ] {
        let box_at = |center| {
            render_core::Aabb::new(center - Vec3::splat(0.01), center + Vec3::splat(0.01)).unwrap()
        };
        assert!(frustum.intersects_aabb(box_at(point)), "{point:?}");
    }
    for point in [
        Vec3::new(-6.0, 0.0, -5.0),
        Vec3::new(6.0, 0.0, -5.0),
        Vec3::new(0.0, -6.0, -5.0),
        Vec3::new(0.0, 6.0, -5.0),
        Vec3::new(0.0, 0.0, -0.75),
        Vec3::new(0.0, 0.0, -11.0),
    ] {
        assert!(
            !frustum.intersects_aabb(
                render_core::Aabb::new(point - Vec3::splat(0.01), point + Vec3::splat(0.01))
                    .unwrap()
            ),
            "{point:?}"
        );
    }
}

#[test]
fn projected_extent_accounts_for_fov_viewport_and_negative_rotated_scale() {
    let mut scene = frontal_scene(-20.0);
    let a = scene
        .prepare(config())
        .draws()
        .next()
        .unwrap()
        .projected_size_px;
    let mut larger = config();
    larger.width *= 2;
    larger.height *= 2;
    let b = scene
        .prepare(larger)
        .draws()
        .next()
        .unwrap()
        .projected_size_px;
    assert!((b - a * 2.0).abs() < 0.001);
    let mut snapshot = scene.snapshot().clone();
    snapshot.projection = render_core::Mat4::perspective_rh(0.5, 1.0, 1.0, 1000.0);
    snapshot.nodes[0].transform.scale = Vec3::new(-2.0, 0.5, 3.0);
    snapshot.nodes[0].transform.rotation = render_core::Quat::from_rotation_y(0.7);
    scene.update(&snapshot).unwrap();
    let prepared = scene.prepare(config());
    assert_eq!(prepared.visible, 1);
    assert!(prepared.draws().next().unwrap().projected_size_px > a);
}

#[test]
fn removal_resets_low_history_and_cached_meshes_are_shared() {
    let mut scene = frontal_scene(-100.0);
    let config = CullingConfig {
        width: 100,
        height: 100,
        low_threshold_px: 8.0,
        high_threshold_px: 20.0,
    };
    let first = scene.prepare(config);
    let second = scene.prepare(config);
    assert!(std::sync::Arc::ptr_eq(
        &first.draws().next().unwrap().mesh,
        &second.draws().next().unwrap().mesh
    ));
    let mut snapshot = scene.snapshot().clone();
    let mut node = snapshot.nodes[0];
    snapshot.nodes.clear();
    scene.update(&snapshot).unwrap();
    node.transform.translation.z = -10.0;
    snapshot.nodes.push(node);
    scene.update(&snapshot).unwrap();
    assert_eq!(
        scene.prepare(config).draws().next().unwrap().lod,
        LodLevel::High
    );
}
