use std::collections::BTreeSet;

use gameplay::GameplayWorld;
use render_core::{
    from_engine, Camera, CameraError, MaterialHandle, MeshHandle, Projection, Quat,
    HudAutopilotState, HudFlightState, HudState, RenderSnapshot, RenderVisualEvent,
    RenderVisualEventKind, SnapshotLight, SnapshotNode,
    Transform, Vec3, VisualAssets,
};

pub const SUN_ENTITY: engine::EntityId = engine::EntityId::new(9_100);
pub const LANDING_INDICATOR_ENTITY: engine::EntityId = engine::EntityId::new(9_205);
pub const ATMOSPHERE_EFFECT_ENTITY: engine::EntityId = engine::EntityId::new(9_206);
pub const IMPACT_FLASH_ENTITY: engine::EntityId = engine::EntityId::new(9_207);
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
pub const ACADEMIC_SURFACE_OBJECT_SEED: u64 = 0xA57A_1E5D;
pub const MAX_ACADEMIC_SURFACE_OBJECTS: usize = 64;
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
pub const STATION_MESH: MeshHandle = MeshHandle::STATION;
pub const DERELICT_MESH: MeshHandle = MeshHandle::DERELICT;
pub const ASTEROID_FIELD_MESH: MeshHandle = MeshHandle::ASTEROID_FIELD;

pub const SHIP_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const ENEMY_MATERIAL: MaterialHandle = VisualAssets::ENEMY;
pub const PLAYER_MATERIAL: MaterialHandle = VisualAssets::PLAYER;
pub const PLACEHOLDER_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const DESTINATION_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const PLANET_MATERIAL: MaterialHandle = VisualAssets::PLANET;
pub const SUN_MATERIAL: MaterialHandle = VisualAssets::SUN;
pub const COLLECTIBLE_MATERIAL: MaterialHandle = VisualAssets::COLLECTIBLE;
pub const STATION_MATERIAL: MaterialHandle = VisualAssets::STATION;
pub const DERELICT_MATERIAL: MaterialHandle = VisualAssets::DERELICT;
pub const ASTEROID_MATERIAL: MaterialHandle = VisualAssets::ASTEROID;

pub fn station_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    let mut nodes = Vec::new();
    let mut seen = BTreeSet::new();
    for interactable in world.interactables() {
        if !matches!(
            interactable.behavior,
            gameplay::InteractionBehavior::Station { .. }
        ) || !seen.insert(interactable.entity_id)
        {
            continue;
        }
        nodes.push(SnapshotNode::new(
            interactable.entity_id,
            Transform::from_translation(from_engine(interactable.position)),
            STATION_MESH,
            STATION_MATERIAL,
            interactable.enabled,
        ));
    }
    nodes
}

pub fn derelict_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    world
        .derelicts()
        .filter(|derelict| derelict.state != gameplay::DerelictState::Undiscovered)
        .map(|derelict| {
            SnapshotNode::derelict(
                derelict.id.value(),
                Transform::from_translation(from_engine(derelict.position)),
                DERELICT_MESH,
                DERELICT_MATERIAL,
                true,
            )
        })
        .collect()
}

pub fn asteroid_field_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    world
        .active_environmental_hazards()
        .filter(|hazard| {
            hazard.hazard_type == gameplay::EnvironmentalHazardType::DebrisField
        })
        .map(|hazard| {
            SnapshotNode::hazard(
                hazard.id.value(),
                Transform::from_translation(from_engine(hazard.position)),
                ASTEROID_FIELD_MESH,
                ASTEROID_MATERIAL,
                true,
            )
        })
        .collect()
}

pub fn academic_surface_objects(elapsed_seconds: f64) -> [SnapshotNode; 4] {
    let objects = academic_surface_objects_with_seed(
        elapsed_seconds,
        ACADEMIC_SURFACE_OBJECT_SEED,
        SURFACE_OBJECT_IDS.len(),
    );
    [objects[0], objects[1], objects[2], objects[3]]
}

pub fn academic_surface_objects_with_seed(
    elapsed_seconds: f64,
    seed: u64,
    count: usize,
) -> Vec<SnapshotNode> {
    let planet = academic_planet_transform(elapsed_seconds);
    let mut rng = SurfaceDistribution::new(seed);
    let mut objects = Vec::with_capacity(count.min(MAX_ACADEMIC_SURFACE_OBJECTS));
    for index in 0..count.min(MAX_ACADEMIC_SURFACE_OBJECTS) {
        let direction = rng.unit_direction();
        let spin = rng.next_f32() * core::f32::consts::TAU;
        let scale = Vec3::new(
            0.08 + rng.next_f32() * 0.08,
            0.1 + rng.next_f32() * 0.12,
            0.08 + rng.next_f32() * 0.08,
        );
        let (mesh, material) = match index % 4 {
            0 => (PLAYER_MESH, PLAYER_MATERIAL),
            1 => (ENEMY_MESH, ENEMY_MATERIAL),
            2 => (DESTINATION_MESH, DESTINATION_MATERIAL),
            _ => (COLLECTIBLE_MESH, COLLECTIBLE_MATERIAL),
        };
        let id = SURFACE_OBJECT_IDS
            .get(index)
            .copied()
            .unwrap_or_else(|| engine::EntityId::new(9_300 + index as u64));
        objects.push(SnapshotNode::new(
            id,
            planet.composed(Transform::new(
                direction * SURFACE_OBJECT_RADIAL_OFFSET,
                surface_object_orientation(direction, spin),
                scale,
            )),
            mesh,
            material,
            true,
        ));
    }
    objects
}

struct SurfaceDistribution {
    state: u64,
}

impl SurfaceDistribution {
    fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(2_685_821_657_736_338_717)
    }

    fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) - 1) as f32
    }

    fn unit_direction(&mut self) -> Vec3 {
        let y = self.next_f32() * 2.0 - 1.0;
        let longitude = self.next_f32() * core::f32::consts::TAU;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        Vec3::new(ring * longitude.cos(), y, ring * longitude.sin()).normalize()
    }
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
    snapshot.simulation_time = world.time().elapsed;
    snapshot
        .nodes
        .reserve(expected_snapshot_node_count(world, params, academic_planet));
    if let Some(fade) = world.impact_fade_to_black() {
        snapshot.overlay.fade_to_black = fade;
        snapshot.overlay.game_over = fade >= 1.0;
    }
    snapshot.overlay.hud = hud_state(world, academic_planet);
    snapshot.visual_events = world
        .visual_events()
        .map(|event| RenderVisualEvent {
            id: event.id,
            kind: match event.kind {
                gameplay::GameplayVisualEventKind::Attack => RenderVisualEventKind::Attack,
                gameplay::GameplayVisualEventKind::EnemyDefeated => {
                    RenderVisualEventKind::EnemyDefeated
                }
                gameplay::GameplayVisualEventKind::ItemCollected => {
                    RenderVisualEventKind::ItemCollected
                }
            },
            source: event.source,
            target: event.target,
            position: event.position.map(from_engine),
            emitted_at: event.emitted_at,
            expires_at: event.expires_at,
        })
        .collect();
    for node in visual_event_nodes(&snapshot) {
        snapshot.push_node(node);
    }

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

    for node in station_nodes(world) {
        snapshot.push_node(node);
    }

    for node in derelict_nodes(world) {
        snapshot.push_node(node);
    }

    for node in asteroid_field_nodes(world) {
        snapshot.push_node(node);
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
        if let Some(planet_id) = academic_planet {
            if world.landing_available(planet_id) {
                let position = world
                    .planet_position_at(planet_id, world.time().elapsed)
                    .map(from_engine)
                    .unwrap_or_else(|| academic_planet_transform(elapsed_seconds).translation);
                snapshot.push_node(SnapshotNode::new(
                    LANDING_INDICATOR_ENTITY,
                    Transform::new(
                        position + Vec3::new(0.0, PLANET_SURFACE_RADIUS + 2.0, 0.0),
                        Quat::IDENTITY,
                        Vec3::splat(0.65),
                    ),
                    DESTINATION_MESH,
                    DESTINATION_MATERIAL,
                    true,
                ));
            }
        }
        for node in academic_surface_objects(elapsed_seconds) {
            snapshot.push_node(node);
        }
    }

    if let Some(ship_position) = visual_ship_position(world) {
        match world.planet_flight_state() {
            gameplay::PlanetFlightState::Atmospheric { intensity, .. } => {
                snapshot.push_node(SnapshotNode::new(
                    ATMOSPHERE_EFFECT_ENTITY,
                    Transform::new(
                        ship_position,
                        Quat::IDENTITY,
                        Vec3::splat(0.35 + intensity.clamp(0.0, 1.0) * 0.8),
                    ),
                    SUN_MESH,
                    SUN_MATERIAL,
                    true,
                ));
            }
            gameplay::PlanetFlightState::Impacted { .. } => {
                if let Some(intensity) = world.impact_flash_intensity() {
                    snapshot.push_node(SnapshotNode::new(
                        IMPACT_FLASH_ENTITY,
                        Transform::new(
                            ship_position,
                            Quat::IDENTITY,
                            Vec3::splat(0.5 + intensity * 1.5),
                        ),
                        SUN_MESH,
                        SUN_MATERIAL,
                        true,
                    ));
                }
            }
            _ => {}
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

fn visual_event_nodes(snapshot: &RenderSnapshot) -> Vec<SnapshotNode> {
    snapshot
        .visual_events
        .iter()
        .filter_map(|event| {
            let position = event.position?;
            let duration = event.expires_at.0.saturating_sub(event.emitted_at.0);
            if duration == 0 || snapshot.simulation_time.0 < event.emitted_at.0 {
                return None;
            }
            let progress = ((snapshot.simulation_time.0 - event.emitted_at.0) as f32
                / duration as f32)
                .clamp(0.0, 1.0);
            let intensity = 1.0 - progress;
            let (mesh, material, scale) = match event.kind {
                RenderVisualEventKind::Attack => (
                    SUN_MESH,
                    SUN_MATERIAL,
                    Vec3::splat(0.2 + intensity * 0.45),
                ),
                RenderVisualEventKind::EnemyDefeated => (
                    SUN_MESH,
                    SUN_MATERIAL,
                    Vec3::splat(0.45 + intensity * 1.4),
                ),
                RenderVisualEventKind::ItemCollected => (
                    COLLECTIBLE_MESH,
                    COLLECTIBLE_MATERIAL,
                    Vec3::splat(0.15 + intensity * 0.35),
                ),
            };
            Some(SnapshotNode::visual_event(
                event.id,
                Transform::new(position, Quat::IDENTITY, scale),
                mesh,
                material,
            ))
        })
        .collect()
}

fn hud_state(
    world: &GameplayWorld,
    academic_planet: Option<gameplay::DestinationId>,
) -> HudState {
    let flight = match world.planet_flight_state() {
        gameplay::PlanetFlightState::Free => HudFlightState::Free,
        gameplay::PlanetFlightState::Atmospheric { .. } => HudFlightState::Atmospheric,
        gameplay::PlanetFlightState::Landing { .. } => HudFlightState::Landing,
        gameplay::PlanetFlightState::Landed { .. } => HudFlightState::Landed,
        gameplay::PlanetFlightState::Impacted { .. } => HudFlightState::Impacted,
    };
    let (autopilot, autopilot_target) = match world.autopilot_state() {
        gameplay::AutopilotState::Inactive => (HudAutopilotState::Inactive, None),
        gameplay::AutopilotState::Planned { destination, .. } => {
            (HudAutopilotState::Planned, Some(destination.value()))
        }
        gameplay::AutopilotState::InTransit { destination, .. } => {
            (HudAutopilotState::InTransit, Some(destination.value()))
        }
        gameplay::AutopilotState::Completed { destination } => {
            (HudAutopilotState::Completed, Some(destination.value()))
        }
        gameplay::AutopilotState::Interrupted { destination } => {
            (HudAutopilotState::Interrupted, Some(destination.value()))
        }
        gameplay::AutopilotState::Failed { destination } => {
            (HudAutopilotState::Failed, Some(destination.value()))
        }
    };
    let target = autopilot_target.or_else(|| match world.travel_state() {
        gameplay::TravelState::InProgress { destination, .. }
        | gameplay::TravelState::Completed { destination }
        | gameplay::TravelState::Cancelled { destination } => Some(destination.value()),
        gameplay::TravelState::Idle => None,
    });
    HudState {
        flight,
        autopilot,
        target,
        active_enemies: world
            .enemies()
            .filter(|enemy| enemy.active && !enemy.is_defeated())
            .count() as u32,
        landing_available: academic_planet.is_some_and(|planet| world.landing_available(planet)),
    }
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
    count += station_nodes(world).len();
    count += derelict_nodes(world).len();
    count += asteroid_field_nodes(world).len();
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
        if academic_planet.is_some_and(|id| world.landing_available(id)) {
            count += 1;
        }
    }
    if matches!(
        world.planet_flight_state(),
        gameplay::PlanetFlightState::Atmospheric { .. }
    ) || world.impact_flash_intensity().is_some()
    {
        if visual_ship_position(world).is_some() {
            count += 1;
        }
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
    let azimuth = if azimuth.is_finite() { azimuth } else { 0.0 };
    let elevation = if elevation.is_finite() {
        elevation.clamp(-1.2, 1.2)
    } else {
        0.0
    };
    let flight_radius = FLIGHT_CAMERA_OFFSET.length();
    let flight_elevation =
        (FLIGHT_CAMERA_OFFSET.y / (FLIGHT_CAMERA_OFFSET.x * FLIGHT_CAMERA_OFFSET.x
            + FLIGHT_CAMERA_OFFSET.z * FLIGHT_CAMERA_OFFSET.z)
            .sqrt())
        .atan()
        + elevation;
    let flight_position = ship
        + Vec3::new(
            flight_radius * flight_elevation.cos() * azimuth.sin(),
            flight_radius * flight_elevation.sin(),
            flight_radius * flight_elevation.cos() * azimuth.cos(),
        );
    let space_position = flight_position;
    let space_target = (SUN_POSITION + ship + planet) / 3.0;
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