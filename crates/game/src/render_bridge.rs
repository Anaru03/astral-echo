use gameplay::GameplayWorld;
use render_core::{
    from_engine, Camera, CameraError, MaterialHandle, MeshHandle, Projection, Quat,
    RenderSnapshot, SnapshotLight, SnapshotNode, Transform, Vec3, VisualAssets,
};

pub const SUN_ENTITY: engine::EntityId = engine::EntityId::new(9_100);
pub const SURFACE_OBJECT_IDS: [engine::EntityId; 4] = [
    engine::EntityId::new(9_201),
    engine::EntityId::new(9_202),
    engine::EntityId::new(9_203),
    engine::EntityId::new(9_204),
];
pub const ACADEMIC_PLANET_DESTINATION_ID: u32 = 9_002;
pub const PLANET_ORBIT_RADIUS: f32 = 42.0;
pub const PLANET_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.25;
pub const PLANET_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND: f32 = 0.5;
pub const PLANET_SURFACE_RADIUS: f32 = 5.0;
pub const SURFACE_OBJECT_RADIAL_OFFSET: f32 = 2.0;
pub const PLANET_OBSERVATION_RADIUS: f32 = 14.0;
pub const OBSERVATION_TRANSITION_DURATION_MS: u64 = 1_000;
const SUN_POSITION: Vec3 = Vec3::new(0.0, 0.0, -18.0);
const FLIGHT_CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 28.0, 46.0);
pub const SHIP_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ENEMY_MESH: MeshHandle = MeshHandle::ENEMY_PLACEHOLDER;
pub const PLAYER_MESH: MeshHandle = MeshHandle::CUBE;
pub const PLACEHOLDER_MESH: MeshHandle = MeshHandle::CUBE;
pub const DESTINATION_MESH: MeshHandle = MeshHandle::DESTINATION_PLACEHOLDER;
pub const PLANET_MESH: MeshHandle = MeshHandle::PLANET_SPHERE;
pub const SUN_MESH: MeshHandle = MeshHandle::SUN_SPHERE;
pub const COLLECTIBLE_MESH: MeshHandle = MeshHandle::COLLECTIBLE_PLACEHOLDER;

pub const SHIP_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const ENEMY_MATERIAL: MaterialHandle = VisualAssets::ENEMY;
pub const PLAYER_MATERIAL: MaterialHandle = VisualAssets::PLAYER;
pub const PLACEHOLDER_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const DESTINATION_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const PLANET_MATERIAL: MaterialHandle = VisualAssets::PLANET;
pub const SUN_MATERIAL: MaterialHandle = VisualAssets::SUN;
pub const COLLECTIBLE_MATERIAL: MaterialHandle = VisualAssets::COLLECTIBLE;

pub fn academic_surface_objects(elapsed_seconds: f64) -> [SnapshotNode; 4] {
    let planet = academic_planet_transform(elapsed_seconds);
    let object_specs = [
        (
            SURFACE_OBJECT_IDS[0],
            PLAYER_MESH,
            PLAYER_MATERIAL,
            Vec3::new(0.0, 1.0, 0.0),
            0.0,
            Vec3::new(0.2, 0.35, 0.2),
        ),
        (
            SURFACE_OBJECT_IDS[1],
            ENEMY_MESH,
            ENEMY_MATERIAL,
            Vec3::new(1.0, 0.0, 0.0),
            -0.35,
            Vec3::new(0.18, 0.48, 0.18),
        ),
        (
            SURFACE_OBJECT_IDS[2],
            DESTINATION_MESH,
            DESTINATION_MATERIAL,
            Vec3::new(0.0, 0.0, 1.0),
            0.55,
            Vec3::new(0.3, 0.22, 0.3),
        ),
        (
            SURFACE_OBJECT_IDS[3],
            COLLECTIBLE_MESH,
            COLLECTIBLE_MATERIAL,
            Vec3::new(-1.0, 0.45, 0.0).normalize(),
            0.8,
            Vec3::splat(0.26),
        ),
    ];
    object_specs
        .map(|(id, mesh, material, direction, spin, scale)| {
            SnapshotNode::new(
                id,
                planet.composed(Transform::new(
                    direction * SURFACE_OBJECT_RADIAL_OFFSET,
                    surface_object_orientation(direction, spin),
                    scale,
                )),
                mesh,
                material,
                true,
            )
        })
}

fn surface_object_orientation(direction: Vec3, spin: f32) -> Quat {
    Quat::from_rotation_arc(Vec3::Y, direction) * Quat::from_rotation_y(spin)
}

#[derive(Clone, Copy, Debug)]
pub struct BridgeParams<'a> {
    pub destinations: &'a [gameplay::DestinationId],
    pub include_collectibles: bool,
}

impl Default for BridgeParams<'_> {
    fn default() -> Self {
        Self {
            destinations: &[],
            include_collectibles: true,
        }
    }
}

pub fn snapshot_from_world(world: &GameplayWorld, params: BridgeParams<'_>) -> RenderSnapshot {
    snapshot_from_world_with_camera(world, params, &Camera::demo())
}

pub fn snapshot_from_world_with_camera(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
) -> RenderSnapshot {
    let tick = world.time().tick;
    let academic_planet = academic_planet_id(world);
    let elapsed_seconds = world.time().elapsed.0 as f64 / 1_000.0;
    let mut snapshot = RenderSnapshot::new(
        tick,
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );
    snapshot
        .nodes
        .reserve(expected_snapshot_node_count(world, params, academic_planet));

    if world.navigation().ship_id().is_some() {
        snapshot.push_node(SnapshotNode::new(
            SUN_ENTITY,
            Transform::new(
                SUN_POSITION,
                Quat::IDENTITY,
                Vec3::splat(8.0),
            ),
            SUN_MESH,
            SUN_MATERIAL,
            true,
        ));
    }

    if let (Some(ship), Some(pos)) = (world.navigation().ship_id(), visual_ship_position(world)) {
        let orientation = world.ship_orientation().unwrap_or_default();
        snapshot.push_node(SnapshotNode::new(
            ship,
            Transform::new(
                pos,
                Quat::from_rotation_y(orientation.yaw_degrees.to_radians())
                    * Quat::from_rotation_x(orientation.pitch_degrees.to_radians()),
                Vec3::ONE,
            ),
            SHIP_MESH,
            SHIP_MATERIAL,
            world.entities().contains(ship),
        ));
    }

    for player in world.players() {
        snapshot.push_node(SnapshotNode::new(
            player.entity_id,
            visual_transform(
                player.position,
                player.orientation.yaw_degrees,
                player.orientation.pitch_degrees,
            ),
            PLAYER_MESH,
            PLAYER_MATERIAL,
            player.is_active && world.entities().contains(player.entity_id),
        ));
    }

    for enemy in world.enemies() {
        if enemy.is_player {
            continue;
        }
        let visible =
            enemy.active && !enemy.is_defeated() && world.entities().contains(enemy.entity);
        snapshot.push_node(SnapshotNode::new(
            enemy.entity,
            Transform::from_translation(from_engine(enemy.position)),
            ENEMY_MESH,
            ENEMY_MATERIAL,
            visible,
        ));
    }

    for dest in world.destinations() {
        if params.destinations.is_empty() || params.destinations.contains(&dest.id) {
            let (mesh, material, scale) = match dest.destination_type {
                gameplay::DestinationType::Planet => (
                    PLANET_MESH,
                    PLANET_MATERIAL,
                    Vec3::splat(5.0),
                ),
                _ => (DESTINATION_MESH, DESTINATION_MATERIAL, Vec3::ONE),
            };
            let transform = if academic_planet == Some(dest.id) {
                let mut transform = academic_planet_transform(elapsed_seconds);
                if let Some(position) = world.planet_position_at(dest.id, world.time().elapsed) {
                    transform.translation = from_engine(position);
                }
                transform
            } else {
                Transform::new(from_engine(dest.position), Quat::IDENTITY, scale)
            };
            snapshot.push_node(SnapshotNode::destination(
                dest.id.value(),
                transform,
                mesh,
                material,
                dest.discovery != gameplay::DiscoveryState::Unknown,
            ));
        }
    }

    let academic_planet_visible = academic_planet.is_some_and(|id| {
        (params.destinations.is_empty() || params.destinations.contains(&id))
            && world
                .destination(id)
                .is_some_and(|destination| destination.discovery != gameplay::DiscoveryState::Unknown)
    });
    if academic_planet_visible {
        for node in academic_surface_objects(elapsed_seconds) {
            snapshot.push_node(node);
        }
    }

    if params.include_collectibles {
        for c in world.collectibles() {
            snapshot.push_node(SnapshotNode::new(
                c.entity,
                Transform::from_translation(from_engine(c.position)),
                COLLECTIBLE_MESH,
                COLLECTIBLE_MATERIAL,
                c.active && world.entities().contains(c.entity),
            ));
        }
    }

    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_point()));
    snapshot.push_light(SnapshotLight::new(render_core::RenderLight::demo_sun()));

    snapshot
}

fn expected_snapshot_node_count(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    academic_planet: Option<gameplay::DestinationId>,
) -> usize {
    let mut count = 0;
    if world.navigation().ship_id().is_some() {
        count += 1;
    }
    if world.navigation().ship_id().is_some() && visual_ship_position(world).is_some() {
        count += 1;
    }
    count += world.players().count();
    count += world.enemies().filter(|enemy| !enemy.is_player).count();
    count += world
        .destinations()
        .filter(|destination| {
            params.destinations.is_empty() || params.destinations.contains(&destination.id)
        })
        .count();

    let academic_planet_visible = academic_planet.is_some_and(|id| {
        (params.destinations.is_empty() || params.destinations.contains(&id))
            && world
                .destination(id)
                .is_some_and(|destination| destination.discovery != gameplay::DiscoveryState::Unknown)
    });
    if academic_planet_visible {
        count += SURFACE_OBJECT_IDS.len();
    }
    if params.include_collectibles {
        count += world.collectibles().count();
    }
    count
}

pub fn visual_ship_position(world: &GameplayWorld) -> Option<Vec3> {
    let current = from_engine(world.ship_position()?);
    let gameplay::TravelState::InProgress { destination, .. } = world.travel_state() else {
        return Some(current);
    };
    if world.planet_position_at(destination, world.time().elapsed).is_some() {
        return Some(current);
    }
    let progress = world.travel_progress()?.progress.clamp(0.0, 1.0);
    let target = world
        .planet_position_at(destination, world.time().elapsed)
        .map(from_engine)
        .or_else(|| world.destination(destination).map(|value| from_engine(value.position)))?;
    Some(interpolate_visual_ship_position(current, target, progress))
}

pub fn interpolate_visual_ship_position(current: Vec3, target: Vec3, progress: f32) -> Vec3 {
    current + (target - current) * progress.clamp(0.0, 1.0)
}

pub fn flight_camera(world: &GameplayWorld, aspect: f32) -> Result<Camera, CameraError> {
    observation_camera(world, aspect, 0.0, 0.0, 0.0)
}

pub fn observation_camera(
    world: &GameplayWorld,
    aspect: f32,
    transition: f32,
    azimuth: f32,
    elevation: f32,
) -> Result<Camera, CameraError> {
    let ship = visual_ship_position(world).unwrap_or(Vec3::ZERO);
    let elapsed_seconds = world.time().elapsed.0 as f64 / 1_000.0;
    let planet = academic_planet_id(world)
        .and_then(|id| world.planet_position_at(id, world.time().elapsed))
        .map(from_engine)
        .unwrap_or_else(|| academic_planet_transform(elapsed_seconds).translation);
    let space_position = ship + FLIGHT_CAMERA_OFFSET;
    let space_target = (SUN_POSITION + ship + planet) / 3.0;
    let elevation = elevation.clamp(-1.2, 1.2);
    let surface_direction = Vec3::new(
        elevation.cos() * azimuth.cos(),
        elevation.sin(),
        elevation.cos() * azimuth.sin(),
    );
    let close_position = planet + surface_direction * PLANET_OBSERVATION_RADIUS;
    let transition = transition.clamp(0.0, 1.0);
    let mut position = space_position.lerp(close_position, transition);
    if let gameplay::PlanetFlightState::Atmospheric { intensity, .. } =
        world.planet_flight_state()
    {
        let phase = elapsed_seconds as f32 * 28.0;
        let amplitude = intensity.clamp(0.0, 1.0) * 0.22;
        position.x += phase.sin() * amplitude;
        position.y += (phase * 1.37).cos() * amplitude;
    }
    let target = space_target.lerp(planet, transition);
    Camera::new(
        position,
        target,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 65.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 500.0,
        },
    )
}

fn academic_planet_id(world: &GameplayWorld) -> Option<gameplay::DestinationId> {
    let planets: Vec<_> = world
        .destinations()
        .filter(|destination| destination.destination_type == gameplay::DestinationType::Planet)
        .map(|destination| destination.id)
        .collect();
    planets
        .iter()
        .copied()
        .find(|id| id.value() == ACADEMIC_PLANET_DESTINATION_ID)
        .or_else(|| {
            if planets.len() == 1 {
                Some(planets[0])
            } else {
                None
            }
        })
}

pub fn academic_planet_transform(elapsed_seconds: f64) -> Transform {
    let orbit_angle = PLANET_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND as f64 * elapsed_seconds;
    let rotation_angle = PLANET_ROTATION_ANGULAR_SPEED_RAD_PER_SECOND as f64 * elapsed_seconds;
    Transform::new(
        SUN_POSITION
            + Vec3::new(
                (orbit_angle.cos() as f32) * PLANET_ORBIT_RADIUS,
                0.0,
                (orbit_angle.sin() as f32) * PLANET_ORBIT_RADIUS,
            ),
        Quat::from_rotation_y(rotation_angle as f32),
        Vec3::splat(5.0),
    )
}

fn visual_transform(position: engine::Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::new(
        from_engine(position),
        Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(pitch.to_radians()),
        Vec3::ONE,
    )
}