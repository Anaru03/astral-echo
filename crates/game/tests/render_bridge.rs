#[path = "../examples/visual_support/mod.rs"]
mod support;

use game::render_bridge::*;
use render_core::{SnapshotId, SnapshotScene, Vec3, VisualAssets};

#[test]
fn maps_all_classes_with_real_ids_and_stable_handles_without_mutation() {
    let world = support::fixture_world();
    let before = format!("{world:?}");
    let snapshot = snapshot_from_world(&world, Default::default());
    assert_eq!(snapshot.nodes.len(), 10);
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node.entity, SnapshotId::Entity(id) if SURFACE_OBJECT_IDS.contains(&id)))
            .count(),
        4
    );
    assert!(snapshot
        .nodes
        .iter()
        .any(|node| node.entity == SnapshotId::Entity(SUN_ENTITY)
            && node.mesh == SUN_MESH
            && node.material == SUN_MATERIAL));
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
        .any(|n| n.entity == SnapshotId::Destination(1) && n.mesh == PLANET_MESH));
    assert!(snapshot.nodes.iter().any(|n| {
        n.entity == SnapshotId::Destination(1)
            && n.mesh == PLANET_MESH
            && n.material == PLANET_MATERIAL
            && n.transform.scale == render_core::Vec3::splat(5.0)
    }));
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
    assert_eq!(scene.visible_nodes().count(), 7);
    assert!(scene.snapshot().nodes.iter().filter(|n| matches!(n.entity,SnapshotId::Entity(id) if id==enemy || id==collectible || id==new_player)).all(|n| !n.visible));
    let empty = gameplay::GameplayWorld::new();
    assert_eq!(
        scene
            .update(&snapshot_from_world(&empty, Default::default()))
            .unwrap()
            .removed,
        11
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
    let collectible = world.collectibles().next().unwrap().entity;
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
        .any(|n| n.entity == SnapshotId::Destination(2) || n.entity == SnapshotId::Entity(collectible)));
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
fn academic_planet_orbit_and_rotation_are_absolute_and_do_not_mutate_world() {
    let world = support::fixture_world();
    let before = format!("{world:?}");
    let at_start = academic_planet_transform(0.0);
    let at_later_time = academic_planet_transform(4.0);
    let at_same_time = academic_planet_transform(4.0);

    let start_radius = (at_start.translation - Vec3::new(0.0, 0.0, -18.0)).length();
    let later_radius = (at_later_time.translation - Vec3::new(0.0, 0.0, -18.0)).length();
    assert!((start_radius - PLANET_ORBIT_RADIUS).abs() < 0.0001);
    assert!((later_radius - PLANET_ORBIT_RADIUS).abs() < 0.0001);
    assert_ne!(at_start.translation, at_later_time.translation);
    assert_ne!(at_start.rotation, at_later_time.rotation);
    assert_eq!(at_later_time, at_same_time);

    let initial_snapshot = snapshot_from_world(&world, Default::default());
    let repeated_snapshot = snapshot_from_world(&world, Default::default());
    let initial_planet = initial_snapshot
        .nodes
        .iter()
        .find(|node| node.entity == SnapshotId::Destination(1))
        .unwrap();
    let repeated_planet = repeated_snapshot
        .nodes
        .iter()
        .find(|node| node.entity == SnapshotId::Destination(1))
        .unwrap();
    assert_eq!(initial_planet.transform, repeated_planet.transform);
    assert_eq!(before, format!("{world:?}"));
}

#[test]
fn academic_surface_objects_are_four_deterministic_exterior_visual_nodes() {
    let world = support::fixture_world();
    let before = format!("{world:?}");
    let first = academic_surface_objects(0.0);
    let repeated = academic_surface_objects(0.0);
    let later = academic_surface_objects(4.0);
    assert_eq!(first, repeated);
    assert_eq!(first.len(), 4);
    assert!(first.iter().all(|node| {
        node.visible
            && node.transform.matrix().is_finite()
            && node.transform.scale.min_element() > 0.0
    }));
    let planet = academic_planet_transform(0.0);
    let later_planet = academic_planet_transform(4.0);
    for (node, later_node) in first.iter().zip(later.iter()) {
        let radius = (node.transform.translation - planet.translation).length();
        let later_radius =
            (later_node.transform.translation - later_planet.translation).length();
        assert!(radius > PLANET_SURFACE_RADIUS);
        assert!((radius - later_radius).abs() < 0.0001);
        assert_ne!(node.transform, later_node.transform);
        assert!(node.mesh != PLANET_MESH);
    }
    let snapshot = snapshot_from_world(&world, Default::default());
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| matches!(node.entity, SnapshotId::Entity(id) if SURFACE_OBJECT_IDS.contains(&id)))
            .count(),
        4
    );
    assert_eq!(before, format!("{world:?}"));
}

#[test]
fn academic_surface_objects_clear_the_planet_with_surface_aligned_axes() {
    let assets = VisualAssets::default();
    let planet = academic_planet_transform(3.75);
    let planet_matrix = planet.matrix();
    let planet_inverse = planet_matrix.inverse();
    let planet_center = planet.translation;
    let nodes = academic_surface_objects(3.75);

    let mut ids = std::collections::BTreeSet::new();
    for node in nodes {
        let id = match node.entity {
            SnapshotId::Entity(id) => id,
            SnapshotId::Destination(_) => panic!("surface object must have a stable entity id"),
        };
        assert!(ids.insert(id), "duplicate surface object id {id:?}");

        let local_object = planet_inverse * node.transform.matrix();
        let local_up = local_object.transform_vector3(Vec3::Y).normalize();
        let local_radial = local_object.transform_point3(Vec3::ZERO).normalize();
        assert!(
            local_up.dot(local_radial) > 0.999,
            "object axis is not aligned with its surface normal: up={local_up:?}, radial={local_radial:?}"
        );

        let mesh = assets.mesh(node.mesh, node.material).unwrap();
        assert!(!mesh.vertices.is_empty());
        for vertex in &mesh.vertices {
            let world_position = node
                .transform
                .matrix()
                .transform_point3(vertex.position);
            let distance = world_position.distance(planet_center);
            assert!(
                distance > PLANET_SURFACE_RADIUS,
                "surface object vertex penetrates planet: distance={distance}"
            );
        }
    }
    assert_eq!(ids.len(), SURFACE_OBJECT_IDS.len());
    assert!(SURFACE_OBJECT_IDS.iter().all(|id| ids.contains(id)));
}

#[test]
fn only_the_selected_academic_planet_orbits() {
    let mut world = support::fixture_world();
    world
        .register_destination(gameplay::Destination {
            id: gameplay::DestinationId::new(ACADEMIC_PLANET_DESTINATION_ID),
            destination_type: gameplay::DestinationType::Planet,
            position: engine::Vec3 {
                x: 70.0,
                y: 3.0,
                z: -12.0,
            },
            travel_duration: engine::Duration(100),
            fuel_cost: 1,
            discovery: gameplay::DiscoveryState::Discovered,
        })
        .unwrap();

    let snapshot = snapshot_from_world(&world, Default::default());
    let selected = snapshot
        .nodes
        .iter()
        .find(|node| node.entity == SnapshotId::Destination(ACADEMIC_PLANET_DESTINATION_ID))
        .unwrap();
    let other = snapshot
        .nodes
        .iter()
        .find(|node| node.entity == SnapshotId::Destination(1))
        .unwrap();
    assert_eq!(selected.transform, academic_planet_transform(0.0));
    assert_eq!(other.transform.translation, Vec3::new(12.0, 0.0, -22.0));
    assert_eq!(other.transform.rotation, render_core::Quat::IDENTITY);
}

#[test]
fn flight_camera_tracks_visual_ship_and_keeps_projection_valid() {
    let world = support::fixture_world();
    let camera = flight_camera(&world, 16.0 / 9.0).unwrap();
    assert!(camera.view_matrix().is_finite());
    assert!(camera.projection_matrix().is_finite());
    assert!(camera.view_projection_matrix().is_finite());
    assert_eq!(visual_ship_position(&world), world.ship_position().map(render_core::from_engine));
}

#[test]
fn observation_camera_stays_outside_planet_and_transition_is_deterministic() {
    let world = support::fixture_world();
    let start = observation_camera(&world, 16.0 / 9.0, 0.0, 0.0, 0.0).unwrap();
    let close = observation_camera(&world, 16.0 / 9.0, 1.0, 0.0, 0.0).unwrap();
    let close_again = observation_camera(&world, 16.0 / 9.0, 1.0, 0.0, 0.0).unwrap();
    let planet = academic_planet_transform(0.0).translation;
    assert!((close.position() - planet).length() > PLANET_SURFACE_RADIUS);
    assert!(start.view_projection_matrix().is_finite());
    assert!(close.view_projection_matrix().is_finite());
    assert_eq!(close, close_again);
    assert!(observation_camera(&world, 16.0 / 9.0, 1.0, 0.0, 1.2)
        .unwrap()
        .view_matrix()
        .is_finite());
}

#[test]
fn observation_transition_reverses_from_current_progress_without_frame_state() {
    let world = support::fixture_world();
    let before = format!("{world:?}");
    let start = observation_camera(&world, 16.0 / 9.0, 0.0, 0.0, 0.0).unwrap();
    let middle = observation_camera(&world, 16.0 / 9.0, 0.5, 0.0, 0.0).unwrap();
    let end = observation_camera(&world, 16.0 / 9.0, 1.0, 0.0, 0.0).unwrap();
    assert_ne!(start.position(), middle.position());
    assert_ne!(middle.position(), end.position());
    assert_eq!(before, format!("{world:?}"));
}

#[test]
fn visual_ship_flight_interpolates_without_frame_based_accumulation() {
    let current = Vec3::ZERO;
    let target = Vec3::new(42.0, 0.0, -18.0);
    let halfway = interpolate_visual_ship_position(current, target, 0.5);
    let repeated = interpolate_visual_ship_position(current, target, 0.5);
    assert_eq!(halfway, Vec3::new(21.0, 0.0, -9.0));
    assert_eq!(halfway, repeated);
    assert_eq!(interpolate_visual_ship_position(current, target, -1.0), current);
    assert_eq!(interpolate_visual_ship_position(current, target, 2.0), target);
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