use std::collections::BTreeSet;

use gameplay::GameplayWorld;
use gameplay::domains::{mission::MissionState, objective::ObjectiveState};
use gameplay::types::MissionId;
use render_core::{
    from_engine, Camera, CameraError, MaterialHandle, MeshHandle, Projection, Quat,
    HudAutopilotState, HudFlightState, HudMissionObjective, HudMissionState, HudObjectiveState,
    HudState, RenderSnapshot, RenderVisualEvent,
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
pub const PLANET_VISUAL_RADIUS: f32 = 8.0;
pub const SUN_VISUAL_RADIUS: f32 = 14.0;
pub const SURFACE_OBJECT_RADIAL_OFFSET: f32 = PLANET_VISUAL_RADIUS + 0.4;
pub const ACADEMIC_SURFACE_OBJECT_SEED: u64 = 0xA57A_1E5D;
pub const MAX_ACADEMIC_SURFACE_OBJECTS: usize = 64;
pub const PLANET_OBSERVATION_RADIUS: f32 = 14.0;
pub const OBSERVATION_TRANSITION_DURATION_MS: u64 = 1_000;
const SUN_POSITION: Vec3 = Vec3::new(0.0, 0.0, -18.0);
const FLIGHT_CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 28.0, 46.0);
pub const SHIP_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ENEMY_MESH: MeshHandle = MeshHandle::ENEMY_PLACEHOLDER;
pub const PLAYER_MESH: MeshHandle = MeshHandle::CUBE;
pub const ERICK_MESH: MeshHandle = MeshHandle::DESTINATION_PLACEHOLDER;
pub const PLACEHOLDER_MESH: MeshHandle = MeshHandle::CUBE;
pub const DESTINATION_MESH: MeshHandle = MeshHandle::DESTINATION_PLACEHOLDER;
pub const PLANET_MESH: MeshHandle = MeshHandle::PLANET_SPHERE;
pub const SUN_MESH: MeshHandle = MeshHandle::SUN_SPHERE;
pub const COLLECTIBLE_MESH: MeshHandle = MeshHandle::COLLECTIBLE_PLACEHOLDER;
pub const STATION_MESH: MeshHandle = MeshHandle::STATION;
pub const DERELICT_MESH: MeshHandle = MeshHandle::DERELICT;
pub const ASTEROID_FIELD_MESH: MeshHandle = MeshHandle::ASTEROID_FIELD;
pub const ECHO7_MESH: MeshHandle = MeshHandle::SHIP_PLACEHOLDER;
pub const ECHO7_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const ECHO7_VISUAL_SCALE: Vec3 = Vec3::new(2.0, 3.0, 1.0);
pub const STATION_VISUAL_SCALE: Vec3 = Vec3::splat(0.35);
pub const ENEMY_SHIP_VISUAL_SCALE: Vec3 = Vec3::splat(0.5);
pub const PLAYER_VISUAL_SCALE: Vec3 = Vec3::splat(0.5);
pub const ECHO7_INTERIOR_HALF_WIDTH: f32 = 9.0;
pub const ECHO7_INTERIOR_HALF_DEPTH: f32 = 7.0;
pub const ECHO7_INTERIOR_FLOOR_Y: f32 = -1.2;
pub const ECHO7_INTERIOR_CEILING_Y: f32 = 6.4;
pub const ECHO7_PLAYER_RADIUS: f32 = 0.45;
pub const INTERIOR_FLOOR_ENTITY: engine::EntityId = engine::EntityId::new(9_401);
pub const INTERIOR_BACK_WALL_ENTITY: engine::EntityId = engine::EntityId::new(9_402);
pub const INTERIOR_LEFT_WALL_ENTITY: engine::EntityId = engine::EntityId::new(9_403);
pub const INTERIOR_RIGHT_WALL_ENTITY: engine::EntityId = engine::EntityId::new(9_404);
pub const INTERIOR_CEILING_ENTITY: engine::EntityId = engine::EntityId::new(9_405);
pub const INTERIOR_LIGHT_ENTITY: engine::EntityId = engine::EntityId::new(9_406);

pub const SHIP_MATERIAL: MaterialHandle = VisualAssets::SHIP;
pub const ENEMY_MATERIAL: MaterialHandle = VisualAssets::ENEMY;
pub const CHARGER_MATERIAL: MaterialHandle = VisualAssets::ENEMY_CHARGER;
pub const SHIELDED_MATERIAL: MaterialHandle = VisualAssets::ENEMY_SHIELDED;
pub const PLAYER_MATERIAL: MaterialHandle = VisualAssets::PLAYER;
pub const ERICK_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const PLACEHOLDER_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const DESTINATION_MATERIAL: MaterialHandle = VisualAssets::DESTINATION;
pub const PLANET_MATERIAL: MaterialHandle = VisualAssets::PLANET;
pub const SUN_MATERIAL: MaterialHandle = VisualAssets::SUN;
pub const COLLECTIBLE_MATERIAL: MaterialHandle = VisualAssets::COLLECTIBLE;
pub const STATION_MATERIAL: MaterialHandle = VisualAssets::STATION;
pub const DERELICT_MATERIAL: MaterialHandle = VisualAssets::DERELICT;
pub const ASTEROID_MATERIAL: MaterialHandle = VisualAssets::ASTEROID;

fn enemy_material(archetype: Option<gameplay::EnemyArchetype>) -> MaterialHandle {
    match archetype {
        Some(gameplay::EnemyArchetype::Charger) => CHARGER_MATERIAL,
        Some(gameplay::EnemyArchetype::Shielded) => SHIELDED_MATERIAL,
        Some(gameplay::EnemyArchetype::Shooter) | None => ENEMY_MATERIAL,
    }
}

pub fn station_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    station_nodes_for_view(world, false)
}

fn station_nodes_for_view(world: &GameplayWorld, interior: bool) -> Vec<SnapshotNode> {
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
            Transform::new(
                from_engine(interactable.position),
                Quat::IDENTITY,
                if interior { Vec3::splat(0.55) } else { STATION_VISUAL_SCALE },
            ),
            if interior { PLACEHOLDER_MESH } else { STATION_MESH },
            if interior { DESTINATION_MATERIAL } else { STATION_MATERIAL },
            interactable.enabled,
        ));
    }
    nodes
}

fn mission_interaction_nodes(world: &GameplayWorld) -> Vec<SnapshotNode> {
    world
        .interactables()
        .filter(|interactable| {
            matches!(
                interactable.behavior,
                gameplay::InteractionBehavior::Console { .. }
                    | gameplay::InteractionBehavior::ElectricalPanel
                    | gameplay::InteractionBehavior::LifeSupportPanel
                    | gameplay::InteractionBehavior::Door { .. }
            )
        })
        .map(|interactable| {
            SnapshotNode::new(
                interactable.entity_id,
                Transform::from_translation(from_engine(interactable.position)),
                PLACEHOLDER_MESH,
                PLACEHOLDER_MATERIAL,
                interactable.enabled,
            )
        })
        .collect()
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
            // This is a planetary surface prop, not a gameplay enemy. Keep
            // enemy identity reserved for world.enemies() so it cannot look
            // like an enemy ship permanently attached to the planet orbit.
            1 => (PLACEHOLDER_MESH, PLACEHOLDER_MATERIAL),
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

/// Selects the mission that the visual presentation should expose.
///
/// Active missions have priority over available missions.  The final fallback
/// is only for presenting the initial locked campaign state; callers that want
/// to start a mission must still require `Available` (or use the legacy
/// first-mission bootstrap in `VisualBridge`).
pub fn presentation_mission(world: &GameplayWorld) -> Option<MissionId> {
    crate::campaign::CAMPAIGN_ORDER
        .iter()
        .copied()
        .find(|mission| {
            world
                .mission(*mission)
                .is_some_and(|value| value.state() == MissionState::Active)
        })
        .or_else(|| {
            crate::campaign::CAMPAIGN_ORDER
                .iter()
                .copied()
                .find(|mission| {
                    world
                        .mission(*mission)
                        .is_some_and(|value| value.state() == MissionState::Available)
                })
        })
        .or_else(|| {
            crate::campaign::CAMPAIGN_ORDER
                .iter()
                .copied()
                .find(|mission| world.mission(*mission).is_some())
        })
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
    snapshot_from_world_with_camera_and_view(world, params, camera, false)
}

pub fn snapshot_from_world_with_camera_and_view(
    world: &GameplayWorld,
    params: BridgeParams<'_>,
    camera: &Camera,
    interior: bool,
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
    let player_defeated = world.players().any(|player| {
        world
            .combatant(player.entity_id)
            .is_some_and(|combatant| combatant.is_defeated())
    });
    if player_defeated {
        snapshot.overlay.game_over = true;
        snapshot.overlay.hud.feedback = Some("JUGADOR DERROTADO".to_owned());
    }
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
                gameplay::GameplayVisualEventKind::Scan => RenderVisualEventKind::Scan,
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

    if interior {
        for node in interior_nodes() {
            snapshot.push_node(node);
        }
    }

    if world.navigation().ship_id().is_some() {
        snapshot.push_node(SnapshotNode::new(
            SUN_ENTITY,
            Transform::new(
                SUN_POSITION,
                Quat::IDENTITY,
                Vec3::splat(SUN_VISUAL_RADIUS),
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
                ECHO7_VISUAL_SCALE,
            ),
            ECHO7_MESH,
            ECHO7_MATERIAL,
            world.entities().contains(ship),
        ));
    }

    for node in station_nodes_for_view(world, interior) {
        snapshot.push_node(node);
    }

    for node in mission_interaction_nodes(world) {
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

    for companion in world.companions() {
        snapshot.push_node(SnapshotNode::new(
            companion.entity,
            Transform::from_translation(from_engine(companion.position)),
            ERICK_MESH,
            ERICK_MATERIAL,
            companion.active && world.entities().contains(companion.entity),
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
            Transform::new(
                from_engine(enemy.position),
                Quat::IDENTITY,
                ENEMY_SHIP_VISUAL_SCALE,
            ),
            ENEMY_MESH,
            enemy_material(enemy.archetype),
            visible,
        ));
    }

    for dest in world.destinations() {
        if params.destinations.is_empty() || params.destinations.contains(&dest.id) {
            let (mesh, material, scale) = match dest.destination_type {
                gameplay::DestinationType::Planet => (
                    PLANET_MESH,
                    PLANET_MATERIAL,
                    Vec3::splat(PLANET_VISUAL_RADIUS),
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
                        position + Vec3::new(0.0, PLANET_VISUAL_RADIUS + 0.8, 0.0),
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

fn interior_nodes() -> [SnapshotNode; 6] {
    [
        SnapshotNode::new(
            INTERIOR_FLOOR_ENTITY,
            Transform::new(
                Vec3::new(0.0, ECHO7_INTERIOR_FLOOR_Y, 0.0),
                Quat::IDENTITY,
                Vec3::new(ECHO7_INTERIOR_HALF_WIDTH, 0.15, ECHO7_INTERIOR_HALF_DEPTH),
            ),
            MeshHandle::CUBE,
            DESTINATION_MATERIAL,
            true,
        ),
        SnapshotNode::new(
            INTERIOR_BACK_WALL_ENTITY,
            Transform::new(
                Vec3::new(0.0, 2.6, -ECHO7_INTERIOR_HALF_DEPTH),
                Quat::IDENTITY,
                Vec3::new(ECHO7_INTERIOR_HALF_WIDTH, 3.8, 0.15),
            ),
            MeshHandle::CUBE,
            DESTINATION_MATERIAL,
            true,
        ),
        SnapshotNode::new(
            INTERIOR_LEFT_WALL_ENTITY,
            Transform::new(
                Vec3::new(-ECHO7_INTERIOR_HALF_WIDTH, 2.6, 0.0),
                Quat::IDENTITY,
                Vec3::new(0.15, 3.8, ECHO7_INTERIOR_HALF_DEPTH),
            ),
            MeshHandle::CUBE,
            DESTINATION_MATERIAL,
            true,
        ),
        SnapshotNode::new(
            INTERIOR_RIGHT_WALL_ENTITY,
            Transform::new(
                Vec3::new(ECHO7_INTERIOR_HALF_WIDTH, 2.6, 0.0),
                Quat::IDENTITY,
                Vec3::new(0.15, 3.8, ECHO7_INTERIOR_HALF_DEPTH),
            ),
            MeshHandle::CUBE,
            DESTINATION_MATERIAL,
            true,
        ),
        SnapshotNode::new(
            INTERIOR_CEILING_ENTITY,
            Transform::new(
                Vec3::new(0.0, ECHO7_INTERIOR_CEILING_Y, 0.0),
                Quat::IDENTITY,
                Vec3::new(ECHO7_INTERIOR_HALF_WIDTH, 0.15, ECHO7_INTERIOR_HALF_DEPTH),
            ),
            MeshHandle::CUBE,
            DESTINATION_MATERIAL,
            true,
        ),
        SnapshotNode::new(
            INTERIOR_LIGHT_ENTITY,
            Transform::new(
                Vec3::new(0.0, 5.8, 0.0),
                Quat::IDENTITY,
                Vec3::new(0.8, 0.15, 0.8),
            ),
            MeshHandle::SUN_SPHERE,
            SUN_MATERIAL,
            true,
        ),
    ]
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
                RenderVisualEventKind::Scan => (
                    DESTINATION_MESH,
                    DESTINATION_MATERIAL,
                    Vec3::splat(0.3 + intensity * 0.8),
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
    let mut completed_mission = None;
    for event in world.events().iter() {
        if let gameplay::GameplayEvent::MissionCompleted { mission, .. } = event {
            completed_mission = Some(*mission);
        }
    }
    let presented_mission = presentation_mission(world);
    let mission = presented_mission.and_then(|mission| world.mission(mission));
    let current_objective = mission.and_then(|value| {
        value
            .objectives()
            .iter()
            .find(|objective| objective.state() == ObjectiveState::Active)
            .or_else(|| {
                value
                    .objectives()
                    .iter()
                    .find(|objective| objective.state() == ObjectiveState::Available)
            })
            .or_else(|| value.objectives().first())
    });
    let mission_progress = mission.map(|value| value.progress());
    let mission_state = mission_progress.map(|value| hud_mission_state(value.state));
    let mission_objectives = mission
        .map(|value| {
            value
                .objectives()
                .iter()
                .map(|objective| HudMissionObjective {
                    id: objective.id.0,
                    name: objective.name.clone(),
                    state: hud_objective_state(objective.state()),
                    progress: objective.progress(),
                    required_progress: objective.required_progress(),
                    required: objective.required,
                })
                .collect()
        })
        .unwrap_or_default();
    HudState {
        flight,
        autopilot,
        target,
        active_enemies: world
            .enemies()
            .filter(|enemy| enemy.active && !enemy.is_defeated())
            .count() as u32,
        landing_available: academic_planet.is_some_and(|planet| world.landing_available(planet)),
        mission_name: mission.map(|value| value.name.clone()),
        mission_description: mission.and_then(|value| value.description.clone()),
        objective_name: current_objective.map(|value| value.name.clone()),
        mission_current_objective: current_objective.map(|value| value.id.0),
        mission_objectives,
        mission_progress: mission_progress.map(|value| {
            (
                value.required_completed as u8,
                value.required_total as u8,
            )
        }),
        mission_state,
        mission_notification: completed_mission
            .or(presented_mission)
            .and_then(|value| mission_notification(world, value)),
        contextual_action: None,
        controls_panel: Default::default(),
        interaction_prompt: None,
        guidance: None,
        controls: None,
        instructions: None,
        feedback: None,
    }
}

fn hud_objective_state(state: ObjectiveState) -> HudObjectiveState {
    match state {
        ObjectiveState::Locked => HudObjectiveState::Locked,
        ObjectiveState::Available => HudObjectiveState::Available,
        ObjectiveState::Active => HudObjectiveState::Active,
        ObjectiveState::Failed => HudObjectiveState::Failed,
        ObjectiveState::Completed => HudObjectiveState::Completed,
    }
}

fn mission_notification(world: &GameplayWorld, mission: MissionId) -> Option<String> {
    let mut selected = None;
    let mut selected_priority = 0;
    for event in world.events().iter() {
        let (priority, text) = match event {
            gameplay::GameplayEvent::MissionStarted { mission: actual, .. }
                if *actual == mission => (3, "NUEVA MISION"),
            gameplay::GameplayEvent::ObjectiveAvailable { objective, .. }
            | gameplay::GameplayEvent::ObjectiveStarted { objective, .. }
                if world.objective(*objective).is_some_and(|value| value.mission_id == mission) =>
            {
                (2, "OBJETIVO ACTUALIZADO")
            }
            gameplay::GameplayEvent::ObjectiveCompleted { objective, .. }
                if world.objective(*objective).is_some_and(|value| value.mission_id == mission) =>
            {
                (4, "OBJETIVO COMPLETADO")
            }
            gameplay::GameplayEvent::MissionCompleted { mission: actual, .. }
                if *actual == mission => (5, "MISION COMPLETADA"),
            _ => continue,
        };
        if priority >= selected_priority {
            selected_priority = priority;
            selected = Some(text.to_owned());
        }
    }
    selected
}

fn hud_mission_state(state: gameplay::domains::mission::MissionState) -> HudMissionState {
    match state {
        gameplay::domains::mission::MissionState::Locked => HudMissionState::Locked,
        gameplay::domains::mission::MissionState::Available => HudMissionState::Available,
        gameplay::domains::mission::MissionState::Active => HudMissionState::Active,
        gameplay::domains::mission::MissionState::Failed => HudMissionState::Failed,
        gameplay::domains::mission::MissionState::Completed => HudMissionState::Completed,
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
    count += world.companions().count();
    count += station_nodes(world).len();
    count += mission_interaction_nodes(world).len();
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

pub fn interior_camera(
    world: &GameplayWorld,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let player = world.players().next().ok_or(CameraError::InvalidAspect)?;
    interior_camera_for_player(world, player.entity_id, aspect)
}

pub fn interior_camera_for_player(
    world: &GameplayWorld,
    player_id: engine::EntityId,
    aspect: f32,
) -> Result<Camera, CameraError> {
    let player = world.player(player_id).ok_or(CameraError::InvalidAspect)?;
    let position = from_engine(player.position) + Vec3::new(0.0, 1.45, 0.0);
    let yaw = player.orientation.yaw_degrees.to_radians();
    let pitch = player.orientation.pitch_degrees.to_radians().clamp(-1.2, 1.2);
    let horizontal = pitch.cos();
    let direction = Vec3::new(yaw.sin() * horizontal, pitch.sin(), -yaw.cos() * horizontal)
        .normalize_or_zero();
    Camera::new(
        position,
        position + direction,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 72.0_f32.to_radians(),
            aspect,
            near: 0.05,
            far: 120.0,
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
        Vec3::splat(PLANET_VISUAL_RADIUS),
    )
}

fn visual_transform(position: engine::Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::new(
        from_engine(position),
        Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(pitch.to_radians()),
        PLAYER_VISUAL_SCALE,
    )
}

#[cfg(test)]
mod tests {
    use super::{academic_planet_transform, hud_mission_state, ECHO7_VISUAL_SCALE,
        PLANET_ORBIT_RADIUS, PLANET_SURFACE_RADIUS, PLANET_VISUAL_RADIUS, SUN_VISUAL_RADIUS};
    use gameplay::{
        commands::GameplayCommand,
        domains::mission::MissionState,
        mission_system::MissionSystem,
        systems::GameplaySystem,
        types::MissionId,
        GameplayWorld,
    };
    use render_core::{HudMissionState, Vec3};

    fn world_with_missions() -> (GameplayWorld, engine::EntityId) {
        let mut world = GameplayWorld::new();
        let player = world.create_player().expect("player should be created");
        world
            .register_mission(crate::el_despertar::definition().expect("awakening is valid"))
            .expect("awakening should register");
        world
            .register_mission(crate::ecos_anomalia::definition().expect("anomaly is valid"))
            .expect("anomaly should register");
        (world, player)
    }

    fn apply(system: &mut MissionSystem, world: &mut GameplayWorld, command: GameplayCommand) {
        let result = system
            .process(world, command)
            .expect("mission command should be handled");
        assert!(matches!(result.status, gameplay::CommandStatus::Success { .. }));
    }

    fn unlock_and_start(
        system: &mut MissionSystem,
        world: &mut GameplayWorld,
        mission: MissionId,
    ) {
        apply(system, world, GameplayCommand::UnlockMission { mission });
        apply(system, world, GameplayCommand::StartMission { mission });
    }

    #[test]
    fn mission_state_conversion_preserves_all_domain_states() {
        let states = [
            (MissionState::Locked, HudMissionState::Locked),
            (MissionState::Available, HudMissionState::Available),
            (MissionState::Active, HudMissionState::Active),
            (MissionState::Failed, HudMissionState::Failed),
            (MissionState::Completed, HudMissionState::Completed),
        ];

        for (domain_state, hud_state) in states {
            assert_eq!(hud_mission_state(domain_state), hud_state);
        }
    }

    #[test]
    fn interior_view_has_a_player_camera_and_spatial_shell() {
        let mut world = GameplayWorld::new();
        let player = world.create_player().expect("player should exist");
        let camera = super::interior_camera(&world, 16.0 / 9.0).expect("camera should be valid");
        assert_eq!(camera.position(), render_core::Vec3::new(0.0, 1.45, 0.0));

        let snapshot = super::snapshot_from_world_with_camera_and_view(
            &world,
            Default::default(),
            &camera,
            true,
        );
        assert!(snapshot.nodes.iter().any(|node| {
            node.entity == render_core::SnapshotId::Entity(super::INTERIOR_FLOOR_ENTITY)
        }));
        assert!(snapshot.nodes.iter().any(|node| {
            node.entity == render_core::SnapshotId::Entity(player)
        }));
        assert!(snapshot.nodes.iter().any(|node| {
            node.entity == render_core::SnapshotId::Entity(super::INTERIOR_BACK_WALL_ENTITY)
        }));
    }

    #[test]
    fn interior_station_representation_is_distinct_from_exterior_station_model() {
        let mut world = GameplayWorld::new();
        let station = world
            .create_station(engine::Vec3::ZERO, 3.0, gameplay::StationType::Piloting)
            .expect("station should register");

        let interior = super::station_nodes_for_view(&world, true);
        let exterior = super::station_nodes_for_view(&world, false);
        assert_eq!(interior.len(), 1);
        assert_eq!(exterior.len(), 1);
        assert_eq!(interior[0].entity, render_core::SnapshotId::Entity(station));
        assert_eq!(exterior[0].entity, render_core::SnapshotId::Entity(station));
        assert_eq!(interior[0].mesh, super::PLACEHOLDER_MESH);
        assert_eq!(interior[0].material, super::DESTINATION_MATERIAL);
        assert_eq!(exterior[0].mesh, super::STATION_MESH);
        assert_eq!(exterior[0].material, super::STATION_MATERIAL);
    }

    #[test]
    fn hud_objective_belongs_to_presented_active_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("El despertar"));
        assert_eq!(hud.objective_name.as_deref(), Some("Evaluar el estado de la nave"));
    }

    #[test]
    fn hud_exposes_authoritative_objective_list_progress_and_transition_notice() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);

        let started = super::hud_state(&world, None);
        assert_eq!(started.mission_description.as_deref(), Some("La tripulación de ECHO-7 despierta tras un incidente desconocido y debe recuperar el control de la nave."));
        assert_eq!(started.mission_current_objective, Some(crate::el_despertar::EVALUAR_ESTADO_NAVE.0));
        assert_eq!(started.mission_objectives.len(), 6);
        assert_eq!(started.mission_objectives[0].id, crate::el_despertar::EVALUAR_ESTADO_NAVE.0);
        assert_eq!(started.mission_objectives[0].progress, 0);
        assert_eq!(started.mission_objectives[0].required_progress, 1);
        assert_eq!(started.mission_notification.as_deref(), Some("NUEVA MISION"));

        apply(&mut system, &mut world, GameplayCommand::StartObjective {
            objective: crate::el_despertar::EVALUAR_ESTADO_NAVE,
        });
        apply(&mut system, &mut world, GameplayCommand::CompleteObjective {
            objective: crate::el_despertar::EVALUAR_ESTADO_NAVE,
        });

        let progressed = super::hud_state(&world, None);
        assert_eq!(progressed.mission_objectives[0].state, render_core::HudObjectiveState::Completed);
        assert_eq!(progressed.mission_objectives[0].progress, 0);
        assert_eq!(progressed.mission_current_objective, Some(crate::el_despertar::RESTAURAR_ENERGIA.0));
        assert_eq!(progressed.objective_name.as_deref(), Some("Restaurar la energía"));
        assert_eq!(progressed.mission_notification.as_deref(), Some("OBJETIVO COMPLETADO"));
        assert_eq!(progressed, super::hud_state(&world, None));

        world.take_events();
        let quiet = super::hud_state(&world, None);
        assert_eq!(quiet.mission_objectives, progressed.mission_objectives);
        assert_eq!(quiet.mission_current_objective, progressed.mission_current_objective);
        assert!(quiet.mission_notification.is_none());
    }

    #[test]
    fn hud_does_not_import_objective_from_another_available_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Available));
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn hud_does_not_import_objective_from_another_active_mission_after_terminal_state() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        for objective in [
            crate::el_despertar::EVALUAR_ESTADO_NAVE,
            crate::el_despertar::RESTAURAR_ENERGIA,
            crate::el_despertar::RESTABLECER_SOPORTE_VITAL,
            crate::el_despertar::ACCEDER_PUENTE_MANDO,
            crate::el_despertar::TOMAR_CONTROL_NAVE,
        ] {
            apply(&mut system, &mut world, GameplayCommand::StartObjective { objective });
            apply(&mut system, &mut world, GameplayCommand::CompleteObjective { objective });
        }
        unlock_and_start(&mut system, &mut world, crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA);

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Active));
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn hud_does_not_import_objective_from_another_active_mission_after_failure() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        apply(
            &mut system,
            &mut world,
            GameplayCommand::FailMission {
                mission: crate::el_despertar::EL_DESPERTAR,
            },
        );
        unlock_and_start(&mut system, &mut world, crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA);

        let hud = super::hud_state(&world, None);

        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Active));
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn hud_keeps_optional_awakening_objective_within_same_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);

        let hud = super::hud_state(&world, None);
        let guidance = world
            .guidance_for_player(_player)
            .expect("guidance should be available");

        assert_eq!(hud.mission_name.as_deref(), Some("El despertar"));
        assert!(guidance.objectives.iter().any(|objective| {
            objective.mission == crate::el_despertar::EL_DESPERTAR
                && !objective.required
                && objective.recommended_action.is_some()
        }));
        assert_eq!(hud.objective_name.as_deref(), Some("Evaluar el estado de la nave"));
    }

    #[test]
    fn presentation_prefers_active_mission_over_another_available_mission() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        assert_eq!(
            super::presentation_mission(&world),
            Some(crate::el_despertar::EL_DESPERTAR)
        );
    }

    #[test]
    fn presentation_selects_first_available_in_campaign_order() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        assert_eq!(
            super::presentation_mission(&world),
            Some(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        );
    }

    #[test]
    fn hud_switches_to_available_successor_without_starting_it() {
        let (mut world, _player) = world_with_missions();
        let mut system = MissionSystem::new();
        unlock_and_start(&mut system, &mut world, crate::el_despertar::EL_DESPERTAR);
        for objective in [
            crate::el_despertar::EVALUAR_ESTADO_NAVE,
            crate::el_despertar::RESTAURAR_ENERGIA,
            crate::el_despertar::RESTABLECER_SOPORTE_VITAL,
            crate::el_despertar::ACCEDER_PUENTE_MANDO,
            crate::el_despertar::TOMAR_CONTROL_NAVE,
        ] {
            apply(&mut system, &mut world, GameplayCommand::StartObjective { objective });
            apply(&mut system, &mut world, GameplayCommand::CompleteObjective { objective });
        }
        apply(
            &mut system,
            &mut world,
            GameplayCommand::UnlockMission {
                mission: crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            },
        );

        let hud = super::hud_state(&world, None);
        assert_eq!(hud.mission_name.as_deref(), Some("Ecos de la anomalía"));
        assert_eq!(hud.mission_state, Some(HudMissionState::Available));
        assert_eq!(hud.mission_notification.as_deref(), Some("MISION COMPLETADA"));
        assert_eq!(world.mission(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA).unwrap().state(), MissionState::Available);
        assert!(hud.objective_name.is_some());
    }

    #[test]
    fn visual_scale_policy_separates_celestial_and_physical_scales() {
        assert!(PLANET_VISUAL_RADIUS >= ECHO7_VISUAL_SCALE.max_element() * 2.0);
        assert!(SUN_VISUAL_RADIUS > PLANET_VISUAL_RADIUS);
        assert_eq!(PLANET_SURFACE_RADIUS, 5.0);
        assert!(PLANET_ORBIT_RADIUS > PLANET_VISUAL_RADIUS * 4.0);
        assert_eq!(academic_planet_transform(0.0).scale, Vec3::splat(PLANET_VISUAL_RADIUS));
    }
}