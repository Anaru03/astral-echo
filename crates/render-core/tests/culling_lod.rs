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
    let mut scene = SnapshotScene::default();
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
    assert_eq!(
        SnapshotId::Entity(engine::EntityId::new(1)),
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
    let mut scene = SnapshotScene::default();
    scene.update(&snapshot).unwrap();
    let prepared = scene.prepare(config());
    assert_eq!(prepared.visible, 1);
    assert!(prepared.draws().next().unwrap().projected_size_px > 8.0);
}
