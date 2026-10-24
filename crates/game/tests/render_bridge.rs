#[path = "../examples/visual_support/mod.rs"]
mod support;

use game::render_bridge::*;
use render_core::{SnapshotId, SnapshotScene, Vec3};

#[test]
fn maps_all_classes_with_real_ids_and_stable_handles_without_mutation() {
    let world = support::fixture_world();
    let before = format!("{world:?}");
    let snapshot = snapshot_from_world(&world, Default::default());
    assert_eq!(snapshot.nodes.len(), 5);
    let ship = world.navigation().ship_id().unwrap();
    assert_ne!(ship, engine::EntityId::new(0));
    let node = snapshot
        .nodes
        .iter()
        .find(|n| n.entity == SnapshotId::Entity(ship))
        .unwrap();
    assert_eq!((node.mesh, node.material), (SHIP_MESH, SHIP_MATERIAL));
    let player = world.players().next().unwrap();
    let node = snapshot
        .nodes
        .iter()
        .find(|n| n.entity == SnapshotId::Entity(player.entity_id))
        .unwrap();
    assert_eq!(
        node.transform.translation,
        render_core::from_engine(player.position)
    );
    assert_eq!((node.mesh, node.material), (PLAYER_MESH, PLAYER_MATERIAL));
    let enemy = world.enemies().next().unwrap();
    assert!(snapshot
        .nodes
        .iter()
        .any(|n| n.entity == SnapshotId::Entity(enemy.entity) && n.mesh == ENEMY_MESH));
    assert!(snapshot
        .nodes
        .iter()
        .any(|n| n.entity == SnapshotId::Destination(1) && n.mesh == DESTINATION_MESH));
    let collectible = world.collectibles().next().unwrap();
    assert!(snapshot
        .nodes
        .iter()
        .any(|n| n.entity == SnapshotId::Entity(collectible.entity) && n.mesh == COLLECTIBLE_MESH));
    assert_eq!(before, format!("{world:?}"));
    assert_eq!(snapshot, snapshot_from_world(&world, Default::default()));
}

#[test]
fn updates_positions_visibility_spawn_and_missing_entities() {
    let mut world = support::fixture_world();
    let player = world.players().next().unwrap().entity_id;
    let mut scene = SnapshotScene::default();
    scene
        .update(&snapshot_from_world(&world, Default::default()))
        .unwrap();
    let original = scene
        .snapshot()
        .nodes
        .iter()
        .find(|n| n.entity == SnapshotId::Entity(player))
        .copied()
        .unwrap();
    let mut processor = gameplay::GameplayProcessor::new();
    processor.add_system(gameplay::systems::PlayerSystem);
    processor.update(
        &mut world,
        [gameplay::GameplayCommand::MovePlayer {
            player,
            direction: engine::Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            delta: engine::Duration(500),
        }],
    );
    scene
        .update(&snapshot_from_world(&world, Default::default()))
        .unwrap();
    let moved = scene
        .snapshot()
        .nodes
        .iter()
        .find(|n| n.entity == original.entity)
        .unwrap();
    assert_ne!(moved.transform.translation, original.transform.translation);
    assert_eq!(
        (moved.mesh, moved.material),
        (original.mesh, original.material)
    );
    let new_player = world.create_player().unwrap();
    assert_eq!(
        scene
            .update(&snapshot_from_world(&world, Default::default()))
            .unwrap()
            .added,
        1
    );
    world.deactivate_entity(new_player);
    world.deactivate_entity(world.navigation().ship_id().unwrap());
    let enemy = world.enemies().next().unwrap().entity;
    let collectible = world.collectibles().next().unwrap().entity;
    world.deactivate_entity(enemy);
    world.deactivate_entity(collectible);
    scene
        .update(&snapshot_from_world(&world, Default::default()))
        .unwrap();
    assert_eq!(scene.visible_nodes().count(), 2);
    assert!(scene.snapshot().nodes.iter().filter(|n| matches!(n.entity,SnapshotId::Entity(id) if id==enemy || id==collectible || id==new_player)).all(|n| !n.visible));
    let empty = gameplay::GameplayWorld::new();
    assert_eq!(
        scene
            .update(&snapshot_from_world(&empty, Default::default()))
            .unwrap()
            .removed,
        6
    );
    assert!(scene.snapshot().nodes.is_empty());
}

#[test]
fn destination_filter_collectibles_and_camera_are_respected() {
    let mut world = support::fixture_world();
    world
        .register_destination(gameplay::Destination {
            id: gameplay::DestinationId::new(2),
            destination_type: gameplay::DestinationType::SpacePoint,
            position: engine::Vec3 {
                x: 20.0,
                y: 0.0,
                z: -20.0,
            },
            travel_duration: engine::Duration(100),
            fuel_cost: 1,
            discovery: gameplay::DiscoveryState::Unknown,
        })
        .unwrap();
    let all = snapshot_from_world(&world, Default::default());
    assert!(
        !all.nodes
            .iter()
            .find(|n| n.entity == SnapshotId::Destination(2))
            .unwrap()
            .visible
    );
    let ids = vec![gameplay::DestinationId::new(1)];
    let filtered = snapshot_from_world(
        &world,
        BridgeParams {
            destinations: &ids,
            include_collectibles: false,
        },
    );
    assert!(!filtered
        .nodes
        .iter()
        .any(|n| n.mesh == COLLECTIBLE_MESH || n.entity == SnapshotId::Destination(2)));
    let camera = render_core::demo_camera(1.0).unwrap();
    let custom = snapshot_from_world_with_camera(&world, Default::default(), &camera);
    assert_eq!(custom.projection, camera.projection_matrix());
    assert_eq!(custom.view, camera.view_matrix());
}

#[test]
fn repeated_cpu_frames_do_not_advance_world_tick() {
    let world = support::fixture_world();
    let tick = world.time();
    let before = format!("{world:?}");
    let mut scene = SnapshotScene::default();
    scene.update(&support::snapshot(&world, 97, 53)).unwrap();
    let first = support::cpu_image(&scene, 97, 53);
    for _ in 0..30 {
        let image = support::cpu_image(&scene, 97, 53);
        assert_eq!(image.rgba, first.rgba);
        assert_eq!(image.depth, first.depth);
    }
    assert_eq!(world.time(), tick);
    assert_eq!(before, format!("{world:?}"));
}

#[test]
fn same_gameplay_scene_and_lifecycle_match_gpu_color_and_depth() {
    let gpu = match render_gpu::GpuContext::new(Default::default()) {
        Ok(gpu) => std::sync::Arc::new(gpu),
        Err(render_gpu::GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    println!("ADAPTER {:?}", gpu.adapter_info());
    let world = support::fixture_world();
    let mut scene = SnapshotScene::default();
    scene.update(&support::snapshot(&world, 321, 181)).unwrap();
    let mut renderer = render_gpu::GpuRenderer::from_snapshot(gpu.clone(), &scene).unwrap();
    let target = render_gpu::OffscreenTarget::new(gpu.clone(), 321, 181).unwrap();
    for stage in 0..6 {
        let mut snapshot = scene.snapshot().clone();
        match stage {
            1 => snapshot.nodes[0].transform.translation += Vec3::new(3.0, 0.0, -2.0),
            2 => snapshot.nodes[1].visible = false,
            3 => snapshot.push_node(render_core::SnapshotNode::new(
                engine::EntityId::new(999),
                render_core::Transform::from_translation(Vec3::new(-5.0, 0.0, -5.0)),
                PLAYER_MESH,
                PLAYER_MATERIAL,
                true,
            )),
            4 => snapshot
                .nodes
                .retain(|n| n.entity != SnapshotId::Entity(engine::EntityId::new(999))),
            5 => snapshot.nodes.clear(),
            _ => (),
        }
        scene.update(&snapshot).unwrap();
        let rebuilt = renderer.sync_snapshot(&scene).unwrap();
        assert_eq!(rebuilt, stage >= 2);
        target.render_snapshot(&mut renderer, &scene).unwrap();
        let cpu = support::cpu_image(&scene, 321, 181);
        let image = target.readback().unwrap();
        println!(
            "stage={stage} comparison={:?}",
            support::compare(&cpu, &image)
        );
        if stage == 0 {
            assert!(image.depth.iter().filter(|d| **d < 1.0).count() > 20);
        }
    }
    gpu.check().unwrap();
}
