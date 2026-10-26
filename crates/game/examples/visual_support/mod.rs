use engine::{Duration, Vec3};
use game::ProjectEchoSession;
use gameplay::{
    Destination, DestinationId, DestinationType, DiscoveryState, EnemyArchetype,
    EnemyConfiguration, GameplayCommand, GameplayProcessor, GameplayWorld, ItemCategory,
    ItemDefinition,
};
use render_core::{Camera, RenderSnapshot, SnapshotScene};
use render_cpu::{Framebuffer, RasterMode};
use render_gpu::GpuImage;

pub fn fixture_world() -> GameplayWorld {
    let mut world = GameplayWorld::new();
    world.start();
    world.create_entity();
    let ship = world.create_entity();
    world.initialize_navigation_ship(ship).unwrap();
    let player = world.create_player().unwrap();
    let mut processor = GameplayProcessor::new();
    processor.add_system(gameplay::systems::PlayerSystem);
    processor.add_system(gameplay::systems::CombatSystem);
    let results = processor.update(
        &mut world,
        [
            GameplayCommand::MovePlayer {
                player,
                direction: Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                delta: Duration(1000),
            },
            GameplayCommand::CreateEnemy {
                configuration: EnemyConfiguration::for_archetype(
                    EnemyArchetype::Shooter,
                    Vec3 {
                        x: -10.0,
                        y: 0.0,
                        z: -12.0,
                    },
                ),
            },
        ],
    );
    assert!(results
        .iter()
        .all(|r| matches!(r.status, gameplay::CommandStatus::Success { .. })));
    world
        .register_destination(Destination {
            id: DestinationId::new(1),
            destination_type: DestinationType::Planet,
            position: Vec3 {
                x: 12.0,
                y: 0.0,
                z: -22.0,
            },
            travel_duration: Duration(1000),
            fuel_cost: 1,
            discovery: DiscoveryState::Known,
        })
        .unwrap();
    let item = gameplay::types::ItemId(70);
    world
        .register_item_definition(ItemDefinition::new(item, ItemCategory::MineralSample, 20))
        .unwrap();
    world
        .create_collectible(
            item,
            1,
            Vec3 {
                x: 9.0,
                y: 0.0,
                z: -9.0,
            },
            2.0,
        )
        .unwrap();
    world
}\ No newline at end of file

pub fn deterministic_flight_session() -> (ProjectEchoSession, DestinationId) {
    let mut session = ProjectEchoSession::new();
    let origin = DestinationId::new(9_001);
    let planet = DestinationId::new(9_002);
    let player = session.ids.first_player;
    let station = session
        .create_station(Vec3::ZERO, 3.0, gameplay::StationType::Piloting)
        .unwrap();
    session
        .register_destination(Destination {
            id: origin,
            destination_type: DestinationType::SpacePoint,
            position: Vec3::ZERO,
            travel_duration: Duration(1_000),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        })
        .unwrap();
    session
        .register_destination(Destination {
            id: planet,
            destination_type: DestinationType::Planet,
            position: Vec3 {
                x: 10.0,
                y: 0.0,
                z: -20.0,
            },
            travel_duration: Duration(1_000),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        })
        .unwrap();
    assert!(session.world_mut().configure_planet_physics(gameplay::PlanetPhysics {
        destination: planet,
        orbit_center: Vec3 {
            x: 0.0,
            y: 0.0,
            z: -20.0,
        },
        orbit_radius: 10.0,
        orbit_angular_speed: 0.1,
        surface_radius: 2.0,
        ship_radius: 1.0,
        gravity_influence_radius: 24.0,
        gravitational_parameter: 80.0,
        atmosphere_height: 4.0,
    }));
    let route = gameplay::NavigationRoute {
        from: origin,
        to: planet,
        distance: 22.36,
        duration: Duration(1_000),
        fuel_cost: 1,
        state: gameplay::NavigationRouteState::Available,
    };
    let results = session.update([
        GameplayCommand::RegisterNavigationRoute { route },
        GameplayCommand::Interact {
            player,
            target: station,
        },
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin,
            destination: planet,
        },
        GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        },
    ]);
    assert!(results.iter().all(|result| {
        matches!(result.status, gameplay::CommandStatus::Success { .. })
    }));
    (session, planet)
}

#[allow(dead_code)]
pub fn deterministic_flight_snapshots(width: u32, height: u32) -> Vec<RenderSnapshot> {
    let (mut session, _) = deterministic_flight_session();
    let mut snapshots = Vec::with_capacity(3);
    snapshots.push(deterministic_flight_snapshot(&session, width, height, 0));
    session.update([GameplayCommand::AdvanceTime {
        delta: Duration(500),
    }]);
    snapshots.push(deterministic_flight_snapshot(&session, width, height, 1));
    snapshots.push(deterministic_flight_snapshot(&session, width, height, 2));
    snapshots
}

#[allow(dead_code)]
pub fn deterministic_impact_snapshots(width: u32, height: u32) -> Vec<RenderSnapshot> {
    let (mut session, planet) = deterministic_flight_session();
    let before = snapshot(session.world(), width, height);
    for _ in 0..20 {
        session.update([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
        if matches!(
            session.world().planet_flight_state(),
            gameplay::PlanetFlightState::Impacted { planet: active } if active == planet
        ) {
            break;
        }
    }
    assert!(matches!(
        session.world().planet_flight_state(),
        gameplay::PlanetFlightState::Impacted { planet: active } if active == planet
    ));
    session.update([GameplayCommand::AdvanceTime {
        delta: Duration(100),
    }]);
    let fade = snapshot(session.world(), width, height);
    session.update([GameplayCommand::AdvanceTime {
        delta: Duration(900),
    }]);
    let game_over = snapshot(session.world(), width, height);
    let mut snapshots = vec![before, fade, game_over];
    for snapshot in &mut snapshots {
        for node in &mut snapshot.nodes {
            if node.entity
                == render_core::SnapshotId::Entity(game::render_bridge::IMPACT_FLASH_ENTITY)
            {
                node.material = render_core::VisualAssets::SUN;
            }
        }
    }
    snapshots
}

#[allow(dead_code)]
pub fn deterministic_flight_snapshot(
    session: &ProjectEchoSession,
    width: u32,
    height: u32,
    stage: usize,
) -> RenderSnapshot {
    let aspect = width as f32 / height.max(1) as f32;
    let camera = if stage == 0 {
        game::render_bridge::flight_camera(session.world(), aspect).unwrap()
    } else {
        game::render_bridge::observation_camera(
            session.world(),
            aspect,
            if stage == 1 { 1.0 } else { 0.0 },
            0.0,
            0.0,
        )
        .unwrap()
    };
    game::render_bridge::snapshot_from_world_with_camera(
        session.world(),
        Default::default(),
        &camera,
    )
}

pub fn snapshot(world: &GameplayWorld, width: u32, height: u32) -> RenderSnapshot {
    let camera = Camera::new(
        render_core::Vec3::new(8.0, 18.0, 26.0),
        render_core::Vec3::new(1.0, 0.0, -10.0),
        render_core::Vec3::Y,
        render_core::Projection::Perspective {
            fov_y_rad: 45.0_f32.to_radians(),
            aspect: width as f32 / height as f32,
            near: 0.1,
            far: 200.0,
        },
    )
    .unwrap();
    game::render_bridge::snapshot_from_world_with_camera(world, Default::default(), &camera)
}

pub fn cpu_image(scene: &SnapshotScene, width: u32, height: u32) -> GpuImage {
    let mut framebuffer = Framebuffer::new(width, height).unwrap();
    render_cpu::render_snapshot_scene(&mut framebuffer, scene, RasterMode::Secuencial).unwrap();
    let rgba = framebuffer.color_bytes().to_vec();
    let depth = framebuffer.depth_slice_mut().to_vec();
    GpuImage {
        width,
        height,
        rgba,
        depth,
    }
}

pub fn compare(cpu: &GpuImage, gpu: &GpuImage) -> (f64, usize, usize) {
    assert_eq!((cpu.width, cpu.height), (gpu.width, gpu.height));
    assert_eq!(cpu.rgba.len(), gpu.rgba.len());
    assert_eq!(cpu.depth.len(), gpu.depth.len());
    let mean = cpu
        .rgba
        .iter()
        .zip(&gpu.rgba)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / cpu.rgba.len() as f64;
    let colors = cpu
        .rgba
        .chunks_exact(4)
        .zip(gpu.rgba.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 2))
        .count();
    let depths = cpu
        .depth
        .iter()
        .zip(&gpu.depth)
        .filter(|(a, b)| !a.is_finite() || !b.is_finite() || (*a - *b).abs() > 1e-5)
        .count();
    assert!(mean <= 0.25, "mean channel error {mean}");
    assert!(
        colors as f64 / cpu.depth.len() as f64 <= 0.005,
        "color outliers {colors}"
    );
    assert!(
        depths as f64 / cpu.depth.len() as f64 <= 0.005,
        "depth outliers {depths}"
    );
    (mean, colors, depths)
}