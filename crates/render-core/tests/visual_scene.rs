use engine::{EntityId, Tick};
use render_core::{
    CullingConfig, MaterialHandle, MeshHandle, RenderSnapshot, SnapshotId, SnapshotNode,
    SnapshotScene, Transform, Vec3, VisualAssets, VisualError,
};

#[test]
fn typed_destination_identity_cannot_replace_real_entity() {
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.push_node(SnapshotNode::destination(
        1,
        Transform::IDENTITY,
        MeshHandle::DESTINATION_PLACEHOLDER,
        VisualAssets::DESTINATION,
        true,
    ));
    assert_eq!(snapshot.nodes.len(), 3);
    assert_eq!(
        snapshot.nodes[0].entity,
        SnapshotId::Entity(EntityId::new(1))
    );
    assert_eq!(snapshot.nodes[2].entity, SnapshotId::Destination(1));
    snapshot.push_node(SnapshotNode::destination(
        1,
        Transform::from_translation(Vec3::X),
        MeshHandle::DESTINATION_PLACEHOLDER,
        VisualAssets::DESTINATION,
        true,
    ));
    assert_eq!(snapshot.nodes.len(), 3);
    assert_eq!(snapshot.nodes[2].transform.translation, Vec3::X);
}

#[test]
fn updates_replace_state_and_remove_missing_entities() {
    let mut scene = SnapshotScene::default();
    let mut snapshot = RenderSnapshot::minimal_demo();
    assert_eq!(scene.update(&snapshot).unwrap().added, 2);
    let identity = snapshot.nodes[0].entity;
    let asset = scene.mesh(&scene.snapshot().nodes[0]).unwrap() as *const _;
    snapshot.nodes[0].transform.translation = Vec3::new(5.0, 2.0, 1.0);
    snapshot.nodes[1].visible = false;
    snapshot.tick = Tick(2);
    let update = scene.update(&snapshot).unwrap();
    assert_eq!((update.added, update.changed, update.removed), (0, 2, 0));
    assert_eq!(scene.visible_nodes().count(), 1);
    assert_eq!(scene.snapshot().nodes[0].entity, identity);
    assert_eq!(
        asset,
        scene.mesh(&scene.snapshot().nodes[0]).unwrap() as *const _
    );
    snapshot.nodes.remove(0);
    assert_eq!(scene.update(&snapshot).unwrap().removed, 1);
    snapshot.nodes.clear();
    assert_eq!(scene.update(&snapshot).unwrap().removed, 1);
    assert!(scene.owned_draws().is_empty());
}

#[test]
fn invalid_snapshot_is_transactional() {
    let mut scene = SnapshotScene::default();
    let snapshot = RenderSnapshot::minimal_demo();
    scene.update(&snapshot).unwrap();
    let mut bad = snapshot.clone();
    bad.nodes[0].material = MaterialHandle::INVALID;
    assert!(matches!(
        scene.update(&bad),
        Err(VisualError::UnknownAsset(..))
    ));
    bad = snapshot.clone();
    bad.nodes.push(bad.nodes[0]);
    assert_eq!(scene.update(&bad), Err(VisualError::DuplicateIdentity));
    bad = snapshot.clone();
    bad.nodes[0].transform.translation.x = f32::NAN;
    assert_eq!(scene.update(&bad), Err(VisualError::NonFinite));
    bad = snapshot.clone();
    bad.view.x_axis.x = f32::INFINITY;
    assert_eq!(scene.update(&bad), Err(VisualError::NonFinite));
    assert_eq!(scene.snapshot(), &snapshot);
}

#[test]
fn order_and_render_sampling_do_not_change_tick_or_handles() {
    let mut scene = SnapshotScene::default();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.nodes.reverse();
    scene.update(&snapshot).unwrap();
    let expected = scene.snapshot().clone();
    for _ in 0..120 {
        assert_eq!(scene.update(&snapshot).unwrap().changed, 0);
        assert_eq!(scene.snapshot(), &expected);
        assert_eq!(scene.owned_draws().len(), 2);
    }
    assert_eq!(scene.snapshot().tick, Tick(1));
}

#[test]
fn procedural_assets_cover_all_visual_classes() {
    let assets = VisualAssets::default();
    for (mesh, material) in [
        (MeshHandle::SHIP_PLACEHOLDER, VisualAssets::SHIP),
        (MeshHandle::CUBE, VisualAssets::PLAYER),
        (MeshHandle::ENEMY_PLACEHOLDER, VisualAssets::ENEMY),
        (
            MeshHandle::DESTINATION_PLACEHOLDER,
            VisualAssets::DESTINATION,
        ),
        (
            MeshHandle::COLLECTIBLE_PLACEHOLDER,
            VisualAssets::COLLECTIBLE,
        ),
        (MeshHandle::SUN_SPHERE, VisualAssets::SUN),
        (MeshHandle::PLANET_SPHERE, VisualAssets::PLANET),
    ] {
        let mesh = assets.mesh(mesh, material).unwrap();
        assert!(!mesh.indices.is_empty());
        assert!(mesh
            .indices
            .iter()
            .all(|i| (*i as usize) < mesh.vertices.len()));
        assert!(mesh
            .vertices
            .iter()
            .all(|v| v.position.is_finite() && v.color.is_finite()));
    }
}

#[test]
fn prepared_scene_stats_match_the_consumed_draws() {
    let mut scene = SnapshotScene::default();
    scene.update(&RenderSnapshot::minimal_demo()).unwrap();
    let prepared = scene.prepare(Default::default());
    let stats = prepared.stats();
    assert_eq!(stats.considered, prepared.considered);
    assert_eq!(stats.visible, prepared.visible);
    assert_eq!(stats.discarded, prepared.discarded);
    assert_eq!(stats.triangles, prepared.triangles);
    assert_eq!(stats.draws, prepared.draws().count());
}

#[test]
fn prepared_scene_stats_are_zero_for_an_empty_scene() {
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.nodes.clear();
    let mut scene = SnapshotScene::default();
    scene.update(&snapshot).unwrap();

    let stats = scene
        .prepare(CullingConfig {
            width: 0,
            height: 0,
            ..Default::default()
        })
        .stats();

    assert_eq!(stats, render_core::PreparedSceneStats::default());
}

#[test]
fn planet_surface_is_deterministic_colored_and_unit_radius() {
    let first_assets = VisualAssets::default();
    let second_assets = VisualAssets::default();
    let first = first_assets
        .mesh(MeshHandle::PLANET_SPHERE, VisualAssets::PLANET)
        .unwrap();
    let second = second_assets
        .mesh(MeshHandle::PLANET_SPHERE, VisualAssets::PLANET)
        .unwrap();
    assert_eq!(first, second);
    assert!(first.vertices.len() > 100);
    assert!(first
        .vertices
        .iter()
        .all(|vertex| (vertex.position.length() - 1.0).abs() < 0.0001));
    assert!(first
        .vertices
        .iter()
        .any(|vertex| vertex.color != first.vertices[0].color));
}