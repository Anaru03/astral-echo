use std::cell::RefCell;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use engine::{Duration, EntityId, Vec3 as GameplayVec3};
use game::ProjectEchoSession;
use gameplay::types::ItemId;
use gameplay::{
    AttackType, BreakerId, BreakerState, Derelict, DerelictId, DerelictReward, Destination,
    DestinationId, DestinationType, DiscoveryState, ElectricalBreakerConfiguration,
    ElectricalPanelConfiguration, EncounterId, EnemyArchetype, EnemyConfiguration,
    EnvironmentalHazard, GameplayCommand, GameplayEvent, InteractionBehavior,
    LifeSupportConfiguration, NavigationRoute, NavigationRouteState,
    OxygenCompartmentConfiguration, PlanetPhysics, RewardDistributionPolicy, SpatialEncounter,
    SpatialEncounterKind, StationType, WeaponAmmunition, WeaponConfiguration,
};
use render_core::{
    GpuAssetState, LightingMode, LodLevel, MaterialHandle, MaterialInfo, MeshHandle,
    VisualAssetRequest, VisualAssetState, VisualAssets,
};
use render_present::{
    run_hot_swap_window_with_input_and_scene_and_focus_and_assets, visual_asset_channel,
    ElementState, GraphicsDebugConfig, KeyCode, Preset, VisualAssetClient, WindowConfig,
};

#[path = "audio.rs"]
mod audio;
#[path = "gamepad.rs"]
mod gamepad;
use audio::{AudioEvent, AudioManager, SoundEffect};
use gamepad::{ControllerButton, GamepadInput, HapticEvent};

const ORIGIN: DestinationId = DestinationId::new(9_001);
const DESTINATION: DestinationId = DestinationId::new(9_002);
const TESSARA_DESTINATION: DestinationId = DestinationId::new(9_601);
const KHARVEX_DESTINATION: DestinationId = DestinationId::new(9_602);
const NARETH_DESTINATION: DestinationId = DestinationId::new(9_603);
const NARETH_POSITION: GameplayVec3 = GameplayVec3 {
    x: 105.30,
    y: 18.0,
    z: -123.30,
};

fn echo7_pick_ray(
    model: &render_core::TexturedModel,
    origin: render_core::Vec3,
    direction: render_core::Vec3,
    transform: render_core::Transform,
) -> Option<(String, String, f32)> {
    let inverse = transform.matrix().inverse();
    let local_origin = inverse.transform_point3(origin);
    let local_direction = inverse.transform_vector3(direction).normalize_or_zero();
    let mut best: Option<(String, String, f32)> = None;
    for part in &model.meshes {
        let material = part
            .material_index
            .and_then(|index| model.materials.get(index))
            .map(|value| value.name.as_str())
            .unwrap_or("<sin material>");
        for triangle in part.mesh.indices.chunks_exact(3) {
            let a = part.mesh.vertices[triangle[0] as usize].position;
            let b = part.mesh.vertices[triangle[1] as usize].position;
            let c = part.mesh.vertices[triangle[2] as usize].position;
            let edge1 = b - a;
            let edge2 = c - a;
            let h = local_direction.cross(edge2);
            let determinant = edge1.dot(h);
            if determinant.abs() < 1e-6 {
                continue;
            }
            let inverse_determinant = determinant.recip();
            let s = local_origin - a;
            let u = inverse_determinant * s.dot(h);
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(edge1);
            let v = inverse_determinant * local_direction.dot(q);
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let distance = inverse_determinant * edge2.dot(q);
            if distance <= 0.0 {
                continue;
            }
            let world_point = transform
                .matrix()
                .transform_point3(local_origin + local_direction * distance);
            let world_distance = world_point.distance(origin);
            if best
                .as_ref()
                .is_none_or(|current| world_distance < current.2)
            {
                best = Some((part.name.clone(), material.to_owned(), world_distance));
            }
        }
    }
    best
}

fn echo7_viewport_ray(
    camera: render_core::Camera,
    pixel_x: f32,
    pixel_y: f32,
    width: f32,
    height: f32,
) -> (render_core::Vec3, render_core::Vec3) {
    let ndc_x = (2.0 * (pixel_x + 0.5) / width) - 1.0;
    let ndc_y = 1.0 - (2.0 * (pixel_y + 0.5) / height);
    let inverse = (camera.projection_matrix() * camera.view_matrix()).inverse();
    let near = inverse * render_core::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far = inverse * render_core::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near = near.truncate() / near.w;
    let far = far.truncate() / far.w;
    (near, (far - near).normalize_or_zero())
}
const NARETH_RESOURCE_ID: &str = "planet.nareth";
const NARETH_MODEL_RELATIVE_PATH: &str = "../../assets/models/seraphim/Seraphim_planet.obj";
const NARETH_MODEL_SHA256: &str =
    "E7CF257E50AE7A51DA3D09E2D69F3313F780616AA23F5026D1649C0783486B93";
const TESSARA_POSITION: GameplayVec3 = GameplayVec3 {
    x: -49.17,
    y: 8.0,
    z: 31.17,
};
const KHARVEX_POSITION: GameplayVec3 = GameplayVec3 {
    x: -70.20,
    y: -12.0,
    z: -88.20,
};
const VISUAL_INITIAL_SHIP_POSITION: GameplayVec3 = GameplayVec3 {
    x: 0.0,
    y: 0.0,
    z: 220.0,
};
const SIGNAL: DestinationId = DestinationId::new(9_003);
const ENCOUNTER: EncounterId = EncounterId::new(9_004);
const OBSERVATION_AZIMUTH_STEP: f32 = 0.2;
const OBSERVATION_ELEVATION_STEP: f32 = 0.1;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FlightKeys {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    ascend: bool,
    descend: bool,
}

impl FlightKeys {
    fn set(&mut self, key: KeyCode, active: bool) -> bool {
        let slot = match key {
            KeyCode::KeyW => &mut self.forward,
            KeyCode::KeyS => &mut self.backward,
            KeyCode::KeyA => &mut self.left,
            KeyCode::KeyD => &mut self.right,
            KeyCode::Space => &mut self.ascend,
            KeyCode::ControlLeft | KeyCode::ControlRight => &mut self.descend,
            _ => return false,
        };
        *slot = active;
        true
    }

    fn input(self) -> GameplayVec3 {
        GameplayVec3 {
            x: if self.right { 1.0 } else { 0.0 } - if self.left { 1.0 } else { 0.0 },
            y: if self.ascend { 1.0 } else { 0.0 } - if self.descend { 1.0 } else { 0.0 },
            z: if self.backward { 1.0 } else { 0.0 } - if self.forward { 1.0 } else { 0.0 },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ObservationTransition {
    from: f32,
    to: f32,
    started_at: u64,
    azimuth: f32,
    elevation: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FeedbackOutcome {
    Complete,
    Partial,
    Failed,
    Empty,
}

#[derive(Debug, Default)]
struct FeedbackAggregate {
    evaluated: usize,
    succeeded: usize,
    failed: usize,
    steps: Vec<String>,
    events: Vec<String>,
}

impl FeedbackAggregate {
    fn record(
        &mut self,
        commands: &[GameplayCommand],
        results: &[gameplay::CommandResult],
        events: impl IntoIterator<Item = String>,
    ) {
        self.evaluated += results.len();
        for (index, result) in results.iter().enumerate() {
            if matches!(result.status, gameplay::CommandStatus::Success { .. }) {
                self.succeeded += 1;
            } else {
                self.failed += 1;
            }
            let command = commands
                .get(index)
                .map(|value| format!("{value:?}"))
                .unwrap_or_else(|| "comando_desconocido".to_owned());
            self.steps.push(format!(
                "paso{}={command}:{:?}",
                self.evaluated - results.len() + index + 1,
                result.status
            ));
        }
        self.events.extend(events);
    }

    fn outcome(&self) -> FeedbackOutcome {
        match (self.succeeded, self.failed) {
            (0, 0) => FeedbackOutcome::Empty,
            (_, 0) => FeedbackOutcome::Complete,
            (0, _) => FeedbackOutcome::Failed,
            _ => FeedbackOutcome::Partial,
        }
    }

    fn is_complete(&self) -> bool {
        self.outcome() == FeedbackOutcome::Complete
    }

    fn message(&self) -> String {
        let label = match self.outcome() {
            FeedbackOutcome::Complete => "OPERACION COMPLETADA",
            FeedbackOutcome::Partial => "OPERACION PARCIALMENTE COMPLETADA",
            FeedbackOutcome::Failed => "OPERACION FALLIDA",
            FeedbackOutcome::Empty => "OPERACION SIN RESULTADO EVALUABLE",
        };
        let events = if self.events.is_empty() {
            "sin eventos".to_owned()
        } else {
            self.events.join(" | ")
        };
        let steps = if self.steps.is_empty() {
            "sin pasos".to_owned()
        } else {
            self.steps.join(" | ")
        };
        format!(
            "{label}: comandos={} exitosos={} fallidos={} pasos={steps} eventos={events}",
            self.evaluated, self.succeeded, self.failed,
        )
    }
}

impl Default for ObservationTransition {
    fn default() -> Self {
        Self {
            from: 0.0,
            to: 0.0,
            started_at: 0,
            azimuth: 0.0,
            elevation: 0.0,
        }
    }
}

impl ObservationTransition {
    fn progress(self, now: u64) -> f32 {
        let elapsed = now.saturating_sub(self.started_at);
        let local = (elapsed as f32
            / game::render_bridge::OBSERVATION_TRANSITION_DURATION_MS as f32)
            .clamp(0.0, 1.0);
        self.from + (self.to - self.from) * local
    }

    fn toggle(&mut self, now: u64) {
        let current = (*self).progress(now);
        self.from = current;
        self.to = if current >= 0.5 { 0.0 } else { 1.0 };
        self.started_at = now;
    }

    fn adjust_orbit(&mut self, azimuth_delta: f32, elevation_delta: f32) {
        self.azimuth += azimuth_delta;
        self.elevation = (self.elevation + elevation_delta).clamp(-1.2, 1.2);
    }

    fn retarget(&mut self, now: u64) {
        self.from = self.progress(now);
        self.to = 1.0;
        self.started_at = now;
    }
}

struct VisualBridge {
    session: ProjectEchoSession,
    player: EntityId,
    ship: EntityId,
    pilot_station: EntityId,
    pilot_exit_local: Option<render_core::Vec3>,
    engineering_station: EntityId,
    status_console: EntityId,
    electrical_panel: EntityId,
    life_support_panel: EntityId,
    bridge_door: EntityId,
    room_doors: Vec<EntityId>,
    enemy: EntityId,
    feedback: String,
    observation: ObservationTransition,
    observation_target: Option<DestinationId>,
    navigation_selector_open: bool,
    free_navigation_candidate: Option<DestinationId>,
    flight_keys: FlightKeys,
    gamepad: GamepadInput,
    audio: AudioManager,
    gamepad_flight: GameplayVec3,
    last_discrete_action: Option<KeyCode>,
    haptic_event_cursor: u64,
    last_impacted: bool,
    last_audio_impacted: bool,
    last_landed: bool,
    last_game_over: bool,
    player_defeated: bool,
    started: bool,
    interior_mode: bool,
    introductory_threat_pending: bool,
    controls_panel_open: bool,
    hud_visible: bool,
    inventory_open: bool,
    pilot_mode: bool,
    pilot_camera_distance: f32,
    surface_mode: bool,
    surface_planet: Option<DestinationId>,
    landing_cinematic_started_at: Option<u64>,
    takeoff_cinematic_started_at: Option<u64>,
    fire_pressed: bool,
    boost_pressed: bool,
    shield_active: bool,
    legacy_exterior_controls: bool,
    look_left: bool,
    look_right: bool,
    look_up: bool,
    look_down: bool,
    last_interior_update: Instant,
    last_world_update: Instant,
    visual_asset_client: Option<VisualAssetClient>,
    visual_asset_states: BTreeMap<String, VisualAssetState>,
    gpu_asset_states: BTreeMap<(MeshHandle, LodLevel), (u64, u64, GpuAssetState)>,
    nareth_diag_snapshot_mesh: Option<MeshHandle>,
    echo7_pick_requested: bool,
}

impl VisualBridge {
    #[cfg(test)]
    fn new() -> Result<Self, String> {
        Self::new_with_mode(false)
    }

    #[cfg(test)]
    fn new_with_mode(interior_mode: bool) -> Result<Self, String> {
        Self::new_with_mode_and_enemy_distance(interior_mode, false, GameplayVec3::ZERO, true)
    }

    fn new_with_mode_and_enemy_distance(
        interior_mode: bool,
        safe_interior_enemy_start: bool,
        initial_ship_position: GameplayVec3,
        legacy_pilot_fixture: bool,
    ) -> Result<Self, String> {
        let mut session = ProjectEchoSession::new();
        let player = session.ids.first_player;
        let ship = session.ids.first_ship;
        let exterior_position = |local: GameplayVec3| GameplayVec3 {
            x: initial_ship_position.x + local.x,
            y: initial_ship_position.y + local.y,
            z: initial_ship_position.z + local.z,
        };
        session.update([
            GameplayCommand::RegisterItemDefinition {
                definition: gameplay::ItemDefinition::new(
                    gameplay::ENERGY_WEAPON_ITEM,
                    gameplay::ItemCategory::Weapon,
                    1,
                ),
            },
            GameplayCommand::RegisterItemDefinition {
                definition: gameplay::ItemDefinition::new(
                    gameplay::PLASMA_WEAPON_ITEM,
                    gameplay::ItemCategory::Weapon,
                    1,
                ),
            },
            GameplayCommand::RegisterDerelict {
                derelict: Derelict::new(
                    DerelictId::new(9_010),
                    exterior_position(GameplayVec3 {
                        x: 70.0,
                        y: 0.0,
                        z: 70.0,
                    }),
                    3.5,
                )
                .with_rewards(
                    vec![DerelictReward {
                        item: gameplay::ENERGY_WEAPON_ITEM,
                        amount: 1,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
            },
            GameplayCommand::RegisterDerelict {
                derelict: Derelict::new(
                    DerelictId::new(9_011),
                    exterior_position(GameplayVec3 {
                        x: -80.0,
                        y: 0.0,
                        z: 40.0,
                    }),
                    3.5,
                )
                .with_rewards(
                    vec![DerelictReward {
                        item: gameplay::PLASMA_WEAPON_ITEM,
                        amount: 1,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
            },
            GameplayCommand::RegisterDerelict {
                derelict: Derelict::new(
                    DerelictId::new(9_012),
                    exterior_position(GameplayVec3 {
                        x: 55.0,
                        y: 0.0,
                        z: -85.0,
                    }),
                    3.5,
                )
                .with_rewards(
                    vec![DerelictReward {
                        item: gameplay::FUEL_RESOURCE_ITEM,
                        amount: 4,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
            },
            GameplayCommand::RegisterDerelict {
                derelict: Derelict::new(
                    DerelictId::new(9_013),
                    exterior_position(GameplayVec3 {
                        x: -70.0,
                        y: 0.0,
                        z: -65.0,
                    }),
                    3.5,
                )
                .with_rewards(
                    vec![DerelictReward {
                        item: gameplay::OXYGEN_RESOURCE_ITEM,
                        amount: 4,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
            },
            GameplayCommand::RegisterDerelict {
                derelict: Derelict::new(
                    DerelictId::new(9_014),
                    exterior_position(GameplayVec3 {
                        x: 95.0,
                        y: 0.0,
                        z: 35.0,
                    }),
                    3.5,
                )
                .with_rewards(
                    vec![DerelictReward {
                        item: gameplay::types::ItemId(1),
                        amount: 2,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
            },
        ]);
        session
            .world_mut()
            .initialize_ship_position(initial_ship_position)
            .map_err(|error| format!("inicializar posición visual de ECHO-7: {error:?}"))?;
        if !session
            .world_mut()
            .initialize_player_position(player, initial_ship_position)
        {
            return Err("inicializar posición del protagonista dentro de ECHO-7".to_owned());
        }
        session
            .register_destination(Destination {
                id: ORIGIN,
                destination_type: DestinationType::SpacePoint,
                position: initial_ship_position,
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .map_err(|error| format!("registrar origen visual: {error:?}"))?;
        session
            .register_destination(Destination {
                id: DESTINATION,
                destination_type: DestinationType::Planet,
                position: GameplayVec3 {
                    x: 42.0,
                    y: 0.0,
                    z: -18.0,
                },
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .map_err(|error| format!("registrar destino visual: {error:?}"))?;
        session
            .register_destination(Destination {
                id: TESSARA_DESTINATION,
                destination_type: DestinationType::Planet,
                position: TESSARA_POSITION,
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .map_err(|error| format!("registrar Tessara: {error:?}"))?;
        session
            .register_destination(Destination {
                id: KHARVEX_DESTINATION,
                destination_type: DestinationType::Planet,
                position: KHARVEX_POSITION,
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .map_err(|error| format!("registrar Kharvex: {error:?}"))?;
        session
            .register_destination(Destination {
                id: NARETH_DESTINATION,
                destination_type: DestinationType::Planet,
                position: NARETH_POSITION,
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Discovered,
            })
            .map_err(|error| format!("registrar Nareth: {error:?}"))?;
        session
            .register_destination(Destination {
                id: SIGNAL,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3 {
                    x: -35.0,
                    y: 8.0,
                    z: -20.0,
                },
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Unknown,
            })
            .map_err(|error| format!("registrar señal visual: {error:?}"))?;
        session.update([GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: ORIGIN,
                to: DESTINATION,
                distance: if initial_ship_position == GameplayVec3::ZERO {
                    49.0
                } else {
                    144.2
                },
                duration: Duration(1_000),
                fuel_cost: 1,
                state: NavigationRouteState::Available,
            },
        }]);
        session.update([GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: DESTINATION,
                to: ORIGIN,
                distance: if initial_ship_position == GameplayVec3::ZERO {
                    49.0
                } else {
                    144.2
                },
                duration: Duration(1_000),
                fuel_cost: 1,
                state: NavigationRouteState::Available,
            },
        }]);
        for (from, to, distance) in [
            (ORIGIN, TESSARA_DESTINATION, 101.85),
            (TESSARA_DESTINATION, ORIGIN, 101.85),
        ] {
            session.update([GameplayCommand::RegisterNavigationRoute {
                route: NavigationRoute {
                    from,
                    to,
                    distance,
                    duration: Duration(1_000),
                    fuel_cost: 1,
                    state: NavigationRouteState::Available,
                },
            }]);
        }
        if !session.world_mut().configure_planet_physics(PlanetPhysics {
            destination: DESTINATION,
            orbit_center: GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: game::render_bridge::PLANET_ORBIT_RADIUS,
            orbit_start: GameplayVec3::ZERO,
            orbit_axis: GameplayVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: game::render_bridge::PLANET_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND,
            surface_radius: game::render_bridge::PLANET_SURFACE_RADIUS,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 24.0,
            atmosphere_height: 8.0,
        }) {
            return Err("configurar la fÃsica planetaria visual".to_owned());
        }
        if !session.world_mut().configure_planet_physics(PlanetPhysics {
            destination: TESSARA_DESTINATION,
            orbit_center: GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: game::render_bridge::TESSARA_ORBIT_RADIUS,
            orbit_start: TESSARA_POSITION,
            orbit_axis: GameplayVec3 {
                x: 49.17,
                y: 0.0,
                z: 49.17,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: game::render_bridge::TESSARA_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND,
            surface_radius: game::render_bridge::TESSARA_VISUAL_RADIUS,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 24.0,
            atmosphere_height: 9.0,
        }) {
            return Err("configurar la física de Tessara".to_owned());
        }
        if !session.world_mut().configure_planet_physics(PlanetPhysics {
            destination: KHARVEX_DESTINATION,
            orbit_center: GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: game::render_bridge::KHARVEX_ORBIT_RADIUS,
            orbit_start: KHARVEX_POSITION,
            orbit_axis: GameplayVec3 {
                x: 70.2,
                y: 0.0,
                z: -70.2,
            },
            orbit_phase: game::render_bridge::KHARVEX_ORBIT_PHASE_RAD,
            orbit_angular_speed: game::render_bridge::KHARVEX_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND,
            surface_radius: game::render_bridge::KHARVEX_SURFACE_RADIUS,
            ship_radius: 1.0,
            gravity_influence_radius: 28.0,
            gravitational_parameter: 30.0,
            atmosphere_height: 10.0,
        }) {
            return Err("configurar la fisica de Kharvex".to_owned());
        }
        if !session.world_mut().configure_planet_physics(PlanetPhysics {
            destination: NARETH_DESTINATION,
            orbit_center: GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: game::render_bridge::NARETH_ORBIT_RADIUS,
            orbit_start: NARETH_POSITION,
            orbit_axis: GameplayVec3 {
                x: 105.3,
                y: 0.0,
                z: 105.3,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: game::render_bridge::NARETH_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND,
            surface_radius: game::render_bridge::NARETH_VISUAL_RADIUS,
            ship_radius: 1.0,
            gravity_influence_radius: 48.0,
            gravitational_parameter: 40.0,
            atmosphere_height: 12.0,
        }) {
            return Err("configurar la fisica de Nareth".to_owned());
        }
        if initial_ship_position != GameplayVec3::ZERO
            && !session.world_mut().configure_solar_physics(PlanetPhysics {
                destination: gameplay::SUN_DESTINATION_ID,
                orbit_center: GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -18.0,
                },
                orbit_radius: 0.0,
                orbit_start: GameplayVec3::ZERO,
                orbit_axis: GameplayVec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                orbit_phase: 0.0,
                orbit_angular_speed: 0.0,
                surface_radius: 12.0,
                ship_radius: 1.0,
                gravity_influence_radius: 80.0,
                gravitational_parameter: 500.0,
                atmosphere_height: 0.0,
            })
        {
            return Err("configurar la física solar visual".to_owned());
        }
        let movement_configured = session.update([GameplayCommand::ConfigureShipMovement {
            ship,
            parameters: gameplay::ShipMovementParameters {
                acceleration: 1.6,
                deceleration: 6.5,
                max_speed: 8.0,
                turn_rate_degrees: 75.0,
            },
        }]);
        if !matches!(
            movement_configured.first().map(|result| &result.status),
            Some(gameplay::CommandStatus::Success { .. })
        ) {
            return Err("configurar el movimiento suave de ECHO-7".to_owned());
        }
        let interior_position = |local: GameplayVec3| GameplayVec3 {
            x: initial_ship_position.x + local.x,
            y: initial_ship_position.y + local.y,
            z: initial_ship_position.z + local.z,
        };
        let pilot_station = if legacy_pilot_fixture {
            session.create_station(
                interior_position(GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.5,
                }),
                3.0,
                StationType::Piloting,
            )
        } else {
            let local_offset = game::render_bridge::echo7_model_to_world(
                game::render_bridge::ECHO7_PILOT_INTERACTION_LOCAL,
                GameplayVec3::ZERO,
                0.0,
            );
            session.create_ship_station(ship, local_offset, 3.0, StationType::Piloting)
        }
        .map_err(|error| format!("crear estaci?n visual: {error:?}"))?;
        let engineering_station = session
            .create_station(
                interior_position(GameplayVec3 {
                    x: 2.5,
                    y: 0.0,
                    z: 0.0,
                }),
                3.0,
                StationType::Engineering,
            )
            .map_err(|error| format!("engineering station: {error:?}"))?;
        let status_console = session
            .create_ship_status_console(
                interior_position(GameplayVec3 {
                    x: -2.0,
                    y: 0.0,
                    z: 0.0,
                }),
                3.0,
            )
            .map_err(|error| format!("crear consola de estado visual: {error:?}"))?;
        let electrical_panel = session
            .create_electrical_panel(
                interior_position(GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -2.0,
                }),
                3.0,
                ElectricalPanelConfiguration::new(vec![ElectricalBreakerConfiguration {
                    id: BreakerId::new(1),
                    initial_state: BreakerState::Off,
                    target_state: BreakerState::On,
                }]),
            )
            .map_err(|error| format!("crear panel electrico visual: {error:?}"))?;
        let life_support_panel = session
            .create_life_support_panel(
                interior_position(GameplayVec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                }),
                3.0,
                LifeSupportConfiguration::new(
                    5,
                    vec![
                        OxygenCompartmentConfiguration {
                            id: gameplay::CompartmentId::new(1),
                            initial_allocation: 0,
                            minimum_allocation: 3,
                            maximum_allocation: 5,
                        },
                        OxygenCompartmentConfiguration {
                            id: gameplay::CompartmentId::new(2),
                            initial_allocation: 0,
                            minimum_allocation: 2,
                            maximum_allocation: 5,
                        },
                    ],
                ),
            )
            .map_err(|error| format!("crear panel de soporte visual: {error:?}"))?;
        let bridge_door = session
            .create_bridge_door(
                interior_position(GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 2.0,
                }),
                3.0,
            )
            .map_err(|error| format!("crear puerta del puente visual: {error:?}"))?;
        let room_doors = if legacy_pilot_fixture {
            Vec::new()
        } else {
            let to_ship_offset =
                |local| game::render_bridge::echo7_model_to_world(local, GameplayVec3::ZERO, 0.0);
            let mut doors = vec![bridge_door];
            session
                .anchor_interactable_to_ship(
                    bridge_door,
                    ship,
                    to_ship_offset(game::render_bridge::ECHO7_ROOM_DOORS[0].control),
                )
                .map_err(|error| format!("anclar puerta de cabina: {error:?}"))?;
            for spec in game::render_bridge::ECHO7_ROOM_DOORS.iter().skip(1) {
                let door = session
                    .create_ship_door(ship, to_ship_offset(spec.control), 3.0)
                    .map_err(|error| format!("crear puerta {}: {error:?}", spec.name))?;
                doors.push(door);
            }
            doors
        };

        let enemy_position = if interior_mode && safe_interior_enemy_start {
            interior_position(GameplayVec3 {
                x: 24.0,
                y: 0.0,
                z: 120.0,
            })
        } else {
            GameplayVec3 {
                x: 18.0,
                y: 0.0,
                z: -18.0,
            }
        };
        let enemy_configuration =
            EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, enemy_position);
        session.update([
            GameplayCommand::ConfigureSignalScanner {
                ship,
                range: 100.0,
                cooldown: Duration(1),
            },
            GameplayCommand::SetSignalScannerEnabled {
                ship,
                enabled: true,
            },
            GameplayCommand::AddInventoryItem {
                player,
                item: ItemId(2),
                amount: 8,
            },
            GameplayCommand::EquipPlayerWeapon {
                player,
                weapon: WeaponConfiguration::with_ammunition(
                    AttackType::Kinetic,
                    20,
                    80,
                    Duration(1_000),
                    WeaponAmmunition {
                        item: ItemId(2),
                        amount_per_attack: 1,
                    },
                ),
            },
            GameplayCommand::RegisterSpatialEncounter {
                encounter: SpatialEncounter::new(
                    ENCOUNTER,
                    SpatialEncounterKind::Hostile {
                        configuration: enemy_configuration,
                    },
                    GameplayVec3::ZERO,
                    4.0,
                ),
            },
        ]);
        if interior_mode {
            session.update([GameplayCommand::CreateEnemy {
                configuration: enemy_configuration,
            }]);
        } else {
            session.update([GameplayCommand::ActivateSpatialEncounter {
                encounter: ENCOUNTER,
            }]);
        }
        let enemy = session
            .world()
            .events()
            .iter()
            .find_map(|event| match event {
                GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
                _ => None,
            })
            .ok_or_else(|| "el escenario visual no creó el enemigo".to_owned())?;
        let introductory_threat_pending = interior_mode;
        if introductory_threat_pending {
            let result = session.update([GameplayCommand::SetEnemyActive {
                enemy,
                active: false,
            }]);
            if !matches!(
                result.first().map(|value| &value.status),
                Some(gameplay::CommandStatus::Success { .. })
            ) {
                return Err("no se pudo proteger la amenaza introductoria".to_owned());
            }
        }
        session.update([GameplayCommand::Interact {
            player,
            target: pilot_station,
        }]);
        session.update([GameplayCommand::SetShipControlActive {
            player,
            pilot_station,
            active: !interior_mode,
        }]);
        let mut audio = AudioManager::new();
        audio.request_exploration_music();
        Ok(Self {
            session,
            player,
            ship,
            pilot_station,
            pilot_exit_local: None,
            engineering_station,
            status_console,
            electrical_panel,
            life_support_panel,
            bridge_door,
            room_doors,
            enemy,
            feedback: "ESCENA INICIALIZADA".to_owned(),
            observation: ObservationTransition::default(),
            observation_target: Some(DESTINATION),
            navigation_selector_open: false,
            free_navigation_candidate: None,
            flight_keys: FlightKeys::default(),
            gamepad: GamepadInput::new(),
            audio,
            gamepad_flight: GameplayVec3::ZERO,
            last_discrete_action: None,
            haptic_event_cursor: 0,
            last_impacted: false,
            last_audio_impacted: false,
            last_landed: false,
            last_game_over: false,
            player_defeated: false,
            started: true,
            interior_mode,
            introductory_threat_pending,
            controls_panel_open: false,
            hud_visible: true,
            inventory_open: false,
            pilot_mode: false,
            pilot_camera_distance: game::render_bridge::PILOT_CAMERA_DEFAULT_DISTANCE,
            surface_mode: false,
            surface_planet: None,
            landing_cinematic_started_at: None,
            takeoff_cinematic_started_at: None,
            fire_pressed: false,
            boost_pressed: false,
            shield_active: false,
            legacy_exterior_controls: !interior_mode,
            look_left: false,
            look_right: false,
            look_up: false,
            look_down: false,
            last_interior_update: Instant::now(),
            last_world_update: Instant::now(),
            visual_asset_client: None,
            visual_asset_states: BTreeMap::new(),
            gpu_asset_states: BTreeMap::new(),
            nareth_diag_snapshot_mesh: None,
            echo7_pick_requested: false,
        })
    }

    fn new_visual_runtime() -> Result<Self, String> {
        let mut bridge = Self::new_with_mode_and_enemy_distance(
            true,
            true,
            VISUAL_INITIAL_SHIP_POSITION,
            false,
        )?;
        bridge.started = false;
        bridge.feedback = "ASTRAL ECHO - PULSA ENTER PARA COMENZAR".to_owned();
        Ok(bridge)
    }

    fn attach_visual_asset_client(&mut self, client: VisualAssetClient) {
        self.visual_asset_client = Some(client);
    }

    fn request_visual_asset(
        &mut self,
        resource_id: impl Into<String>,
        path: impl Into<std::path::PathBuf>,
        mesh: MeshHandle,
        material: MaterialHandle,
    ) -> bool {
        let resource_id = resource_id.into();
        if self.visual_asset_states.contains_key(&resource_id) {
            return false;
        }
        let request = VisualAssetRequest {
            resource_id: resource_id.clone(),
            path: path.into(),
            mesh,
            material,
        };
        let Some(client) = self.visual_asset_client.as_ref() else {
            return false;
        };
        if client.request(request).is_err() {
            return false;
        }
        self.visual_asset_states
            .insert(resource_id, VisualAssetState::Loading);
        true
    }

    fn poll_visual_asset_events(&mut self) {
        let Some(client) = self.visual_asset_client.as_mut() else {
            return;
        };
        for event in client.poll_events() {
            if event.resource_id == NARETH_RESOURCE_ID {
                let label = match event.state {
                    VisualAssetState::Loading => "FALLBACK_LOADING_CPU",
                    VisualAssetState::Ready => "FALLBACK_CPU_REGISTERED_GPU_PENDING",
                    VisualAssetState::Failed => "FALLBACK_CPU_FAILED",
                };
                eprintln!(
                    "nareth-diag state={} destination=9603 resource={} error={:?}",
                    label, event.resource_id, event.error
                );
            }
            self.visual_asset_states
                .insert(event.resource_id, event.state.clone());
            if let VisualAssetState::Failed = event.state {
                if let Some(error) = event.error {
                    self.feedback = format!("RECURSO VISUAL NO DISPONIBLE: {error}");
                }
            }
        }
        for event in client.poll_gpu_events() {
            let key = (event.mesh, event.lod);
            let is_newer = self
                .gpu_asset_states
                .get(&key)
                .is_none_or(|(epoch, generation, _)| {
                    (event.renderer_epoch, event.generation) >= (*epoch, *generation)
                });
            if is_newer {
                let state = event.state.clone();
                if event.mesh == MeshHandle::NARETH_PLANET {
                    let label = match state {
                        GpuAssetState::Unavailable => "FALLBACK_GPU_FAILED",
                        GpuAssetState::UploadingGpu => "FALLBACK_CPU_REGISTERED_GPU_PENDING",
                        GpuAssetState::GpuReady => "REAL_MODEL_SELECTED",
                        GpuAssetState::GpuFailed => "FALLBACK_GPU_FAILED",
                    };
                    eprintln!(
                        "nareth-diag state={} destination=9603 resource={} mesh={} lod={:?} renderer_epoch={} generation={} error={:?}",
                        label, NARETH_RESOURCE_ID, event.mesh.0, event.lod,
                        event.renderer_epoch, event.generation, event.error
                    );
                }
                self.gpu_asset_states
                    .insert(key, (event.renderer_epoch, event.generation, state.clone()));
                if event.mesh == MeshHandle::NARETH_PLANET
                    && matches!(state, GpuAssetState::GpuFailed)
                {
                    if let Some(error) = event.error {
                        self.feedback = format!("RECURSO GPU NO DISPONIBLE: {error}");
                    }
                }
            } else if event.mesh == MeshHandle::NARETH_PLANET {
                eprintln!(
                    "nareth-diag state=FALLBACK_STALE_GENERATION destination=9603 resource={} mesh={} lod={:?} renderer_epoch={} generation={}",
                    NARETH_RESOURCE_ID, event.mesh.0, event.lod,
                    event.renderer_epoch, event.generation
                );
            }
        }
    }

    fn log_nareth_snapshot_mesh(&mut self, mesh: MeshHandle) {
        if self.nareth_diag_snapshot_mesh == Some(mesh) {
            return;
        }
        self.nareth_diag_snapshot_mesh = Some(mesh);
        let label = if mesh == MeshHandle::NARETH_PLANET {
            "REAL_MODEL_SELECTED"
        } else {
            match self.visual_asset_states.get(NARETH_RESOURCE_ID) {
                None => "FALLBACK_NOT_REQUESTED",
                Some(VisualAssetState::Loading) => "FALLBACK_LOADING_CPU",
                Some(VisualAssetState::Failed) => "FALLBACK_CPU_FAILED",
                Some(VisualAssetState::Ready) => match self
                    .gpu_asset_states
                    .get(&(MeshHandle::NARETH_PLANET, LodLevel::High))
                    .map(|(_, _, state)| state)
                {
                    Some(GpuAssetState::GpuFailed | GpuAssetState::Unavailable) => {
                        "FALLBACK_GPU_FAILED"
                    }
                    _ => "FALLBACK_CPU_REGISTERED_GPU_PENDING",
                },
            }
        };
        eprintln!(
            "nareth-diag state={} destination=9603 resource={} snapshot_mesh={} snapshot_material={}",
            label, NARETH_RESOURCE_ID, mesh.0, VisualAssets::PLANET.0
        );
    }

    fn nareth_visual_asset_ready(&self) -> bool {
        self.visual_asset_states
            .get(NARETH_RESOURCE_ID)
            .is_some_and(|state| *state == VisualAssetState::Ready)
            && self
                .gpu_asset_states
                .get(&(MeshHandle::NARETH_PLANET, LodLevel::High))
                .is_some_and(|(_, _, state)| *state == GpuAssetState::GpuReady)
    }

    fn request_nareth_visual_asset(&mut self) {
        let request = nareth_visual_asset_request();
        let size = std::fs::metadata(&request.path)
            .map(|metadata| metadata.len())
            .ok();
        let resource_id = request.resource_id.clone();
        let path = request.path.clone();
        let requested = self.request_visual_asset(
            request.resource_id,
            request.path,
            request.mesh,
            request.material,
        );
        eprintln!(
            "nareth-diag state={} destination=9603 resource={} mesh={} material={} path={} bytes={:?} sha256_verified={} sha256_source=preflight request_accepted={}",
            if requested { "FALLBACK_LOADING_CPU" } else { "FALLBACK_CPU_FAILED" },
            resource_id, request.mesh.0, request.material.0,
            path.display(), size, NARETH_MODEL_SHA256, requested
        );
    }

    fn start_game(&mut self) {
        if self.started || self.player_defeated {
            return;
        }
        self.started = true;
        self.last_world_update = Instant::now();
        self.last_interior_update = Instant::now();
        self.feedback = "DESPERTAR EN ECHO-7 - PRESIONA M PARA INICIAR EL DESPERTAR".to_owned();
        println!("visual gameplay: {}", self.feedback);
    }

    fn advance_visual_time(&mut self) {
        let now = Instant::now();
        let delta_ms = now
            .saturating_duration_since(self.last_world_update)
            .as_millis()
            .max(1) as u64;
        self.last_world_update = now;
        if self.started && !self.player_defeated {
            self.advance_visual_time_by(Duration(delta_ms));
        }
    }

    fn advance_visual_time_by(&mut self, delta: Duration) {
        if !self.started || self.player_defeated {
            return;
        }
        self.session
            .update([GameplayCommand::AdvanceTime { delta }]);
        self.update_planet_surface_transition();
        self.activate_introductory_threat_if_ready();
        self.refresh_player_defeat_state();
    }

    fn issue(&mut self, commands: impl IntoIterator<Item = GameplayCommand>) -> bool {
        if self.player_defeated {
            return false;
        }
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into(commands, &mut aggregate);
        self.finish_issue(aggregate)
    }

    fn issue_into(
        &mut self,
        commands: impl IntoIterator<Item = GameplayCommand>,
        aggregate: &mut FeedbackAggregate,
    ) -> bool {
        if self.player_defeated {
            return false;
        }
        let commands: Vec<_> = commands.into_iter().collect();
        let results = self.session.update(commands.clone());
        self.activate_introductory_threat_if_ready();
        self.refresh_player_defeat_state();
        let accepted = results
            .iter()
            .all(|result| matches!(result.status, gameplay::CommandStatus::Success { .. }));
        let events = self
            .session
            .world()
            .events()
            .iter()
            .map(|value| format!("{:?}", value));
        aggregate.record(&commands, &results, events);
        accepted
    }

    fn activate_introductory_threat_if_ready(&mut self) {
        if !self.introductory_threat_pending
            || !self
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .is_some_and(|mission| {
                    mission.state == gameplay::domains::mission::MissionState::Completed
                })
        {
            return;
        }
        let Some(enemy) = self.session.world().combatant(self.enemy).copied() else {
            return;
        };
        if enemy.is_defeated() {
            self.introductory_threat_pending = false;
            return;
        }
        let result = self.session.update([GameplayCommand::SetEnemyActive {
            enemy: self.enemy,
            active: true,
        }]);
        if matches!(
            result.first().map(|value| &value.status),
            Some(gameplay::CommandStatus::Success { .. })
        ) {
            self.introductory_threat_pending = false;
        }
    }

    fn finish_issue(&mut self, aggregate: FeedbackAggregate) -> bool {
        if self.player_defeated {
            self.feedback = "JUGADOR DERROTADO — SESION VISUAL DETENIDA".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return false;
        }
        let succeeded = aggregate.is_complete();
        self.feedback = aggregate.message();
        println!("visual gameplay: {}", self.feedback);
        succeeded
    }

    fn refresh_player_defeat_state(&mut self) {
        if self.player_defeated {
            return;
        }
        let defeated = self.session.world().events().iter().any(|event| {
            matches!(event, GameplayEvent::PlayerDefeated { player } if *player == self.player)
        }) || self.session.world().ship_is_destroyed(self.ship) == Some(true);
        if defeated {
            self.player_defeated = true;
            self.feedback = "JUGADOR DERROTADO — SESION VISUAL DETENIDA".to_owned();
            println!("visual gameplay: {}", self.feedback);
        }
    }

    fn handle_key(&mut self, key: KeyCode, state: ElementState) {
        if !self.started {
            if state == ElementState::Pressed && key == KeyCode::Enter {
                self.start_game();
            }
            return;
        }
        if state == ElementState::Pressed && key == KeyCode::F1 {
            if !self.player_defeated {
                self.controls_panel_open = !self.controls_panel_open;
                self.flight_keys = FlightKeys::default();
                self.look_left = false;
                self.look_right = false;
                self.look_up = false;
                self.look_down = false;
            }
            return;
        }
        if state == ElementState::Pressed && key == KeyCode::F2 {
            self.hud_visible = !self.hud_visible;
            self.controls_panel_open = false;
            return;
        }
        if state == ElementState::Pressed && key == KeyCode::F3 {
            self.inventory_open = !self.inventory_open;
            return;
        }
        if state == ElementState::Pressed && key == KeyCode::F4 {
            self.echo7_pick_requested = true;
            return;
        }
        if self.controls_panel_open {
            return;
        }
        if self.pilot_mode && state == ElementState::Pressed {
            match key {
                KeyCode::PageUp | KeyCode::Equal => {
                    self.pilot_camera_distance = (self.pilot_camera_distance - 2.0)
                        .max(game::render_bridge::PILOT_CAMERA_MIN_DISTANCE);
                    return;
                }
                KeyCode::PageDown | KeyCode::Minus => {
                    self.pilot_camera_distance = (self.pilot_camera_distance + 2.0)
                        .min(game::render_bridge::PILOT_CAMERA_MAX_DISTANCE);
                    return;
                }
                _ => {}
            }
        }
        if self.pilot_mode && state == ElementState::Pressed && key == KeyCode::KeyN {
            self.toggle_navigation_selector();
            return;
        }
        if self.navigation_selector_open {
            if state != ElementState::Pressed {
                return;
            }
            match key {
                KeyCode::ArrowLeft => self.select_free_navigation_candidate(-1),
                KeyCode::ArrowRight => self.select_free_navigation_candidate(1),
                KeyCode::Enter => self.confirm_free_navigation_candidate(),
                _ => {}
            }
            return;
        }
        let player = self.player;
        let ship = self.ship;
        let enemy = self.enemy;
        let pilot_station = self.pilot_station;
        if self.pilot_mode
            && state == ElementState::Pressed
            && key == KeyCode::Escape
            && matches!(
                self.session.world().travel_state(),
                gameplay::TravelState::InProgress { .. }
            )
        {
            if self.issue([GameplayCommand::CancelAutopilot {
                player,
                pilot_station,
            }]) {
                self.feedback = "ERICK: autopilotaje libre cancelado.".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
            return;
        }
        if self.player_defeated {
            if state == ElementState::Pressed {
                match key {
                    KeyCode::KeyO => self.toggle_observation(),
                    KeyCode::ArrowLeft => self
                        .observation
                        .adjust_orbit(-OBSERVATION_AZIMUTH_STEP, 0.0),
                    KeyCode::ArrowRight => {
                        self.observation.adjust_orbit(OBSERVATION_AZIMUTH_STEP, 0.0)
                    }
                    KeyCode::ArrowUp => self
                        .observation
                        .adjust_orbit(0.0, OBSERVATION_ELEVATION_STEP),
                    KeyCode::ArrowDown => self
                        .observation
                        .adjust_orbit(0.0, -OBSERVATION_ELEVATION_STEP),
                    _ => {}
                }
            }
            return;
        }
        if self.flight_keys.set(key, state == ElementState::Pressed) {
            if self.ship_flight_controls_enabled() {
                self.accelerate_ship(player, pilot_station, self.current_flight_input());
            }
            return;
        }
        if matches!(key, KeyCode::ShiftLeft | KeyCode::ShiftRight) && self.pilot_mode {
            self.boost_pressed = state == ElementState::Pressed;
            self.issue([GameplayCommand::SetShipBoost {
                player,
                pilot_station,
                active: self.boost_pressed,
            }]);
            return;
        }
        if self.pilot_mode && self.observation_is_active() && state == ElementState::Pressed {
            match key {
                KeyCode::BracketLeft => {
                    self.select_observation_planet(-1);
                    return;
                }
                KeyCode::BracketRight => {
                    self.select_observation_planet(1);
                    return;
                }
                _ => {}
            }
        }
        if key == KeyCode::KeyG && self.pilot_mode {
            if state == ElementState::Pressed {
                let active = !self.shield_active;
                if self.issue([GameplayCommand::SetShipShieldActive {
                    player,
                    ship,
                    active,
                }]) {
                    self.shield_active = active;
                }
            }
            return;
        }
        if key == KeyCode::KeyX && self.pilot_mode {
            if state == ElementState::Pressed {
                self.issue([GameplayCommand::BrakeShip {
                    player,
                    pilot_station,
                }]);
            }
            return;
        }
        if key == KeyCode::KeyF && self.pilot_mode {
            self.fire_pressed = state == ElementState::Pressed;
            if self.fire_pressed {
                self.issue([GameplayCommand::AttackEnemy { player, enemy }]);
            }
            return;
        }
        if (self.interior_mode || self.pilot_mode) && state == ElementState::Released {
            match key {
                KeyCode::ArrowLeft => self.look_left = false,
                KeyCode::ArrowRight => self.look_right = false,
                KeyCode::ArrowUp => self.look_up = false,
                KeyCode::ArrowDown => self.look_down = false,
                _ => {}
            }
            return;
        }
        if state != ElementState::Pressed {
            return;
        }
        self.last_discrete_action = Some(key);
        match key {
            KeyCode::KeyM => self.start_presented_mission(),
            KeyCode::KeyT => self.retry_cooperative_mission_content(),
            KeyCode::KeyI if self.pilot_mode => self.exit_pilot(),
            KeyCode::KeyI if self.surface_mode => self.try_takeoff(),
            KeyCode::KeyI => self.guided_interaction(),
            KeyCode::KeyL => self.request_landing_from_current_position(),
            KeyCode::KeyR => self.restore_energy(),
            KeyCode::KeyY => self.restore_life_support(),
            KeyCode::KeyU if self.pilot_mode => self.use_fuel_resource(),
            KeyCode::Space => {
                self.issue([GameplayCommand::AdvanceTime {
                    delta: Duration(100),
                }]);
                self.last_world_update = Instant::now();
            }
            KeyCode::KeyQ => {
                if self.issue([GameplayCommand::ScanSignals { player, ship }]) {
                    self.audio.request_effect(SoundEffect::Scan);
                }
            }
            KeyCode::KeyV => self.equip_energy_weapon(),
            KeyCode::KeyB => self.equip_found_weapon(gameplay::PLASMA_WEAPON_ITEM),
            KeyCode::KeyC => self.equip_kinetic_weapon(),
            KeyCode::KeyF => {
                if self.landing_target().is_some() {
                    self.request_landing_from_current_position();
                } else if self.ship_flight_controls_enabled() {
                    self.issue([GameplayCommand::AttackEnemy { player, enemy }]);
                }
            }
            KeyCode::KeyE => {
                let collectible = self
                    .session
                    .world()
                    .collectibles()
                    .find(|value| value.active)
                    .map(|value| (value.entity, value.amount));
                if let Some((collectible, amount)) = collectible {
                    self.issue([GameplayCommand::CollectItem {
                        player,
                        collectible,
                        amount,
                    }]);
                }
            }
            KeyCode::KeyP if self.ship_flight_controls_enabled() => {
                self.toggle_flight(player, pilot_station)
            }
            KeyCode::KeyO => self.toggle_observation(),
            KeyCode::ArrowLeft if self.interior_mode || self.pilot_mode => {
                self.look_left = true;
                if self.interior_mode {
                    self.rotate_player(-6.0, 0.0);
                }
                if self.pilot_mode {
                    self.rotate_ship_from_input(1.0, 0.0);
                }
            }
            KeyCode::ArrowRight if self.interior_mode || self.pilot_mode => {
                self.look_right = true;
                if self.interior_mode {
                    self.rotate_player(6.0, 0.0);
                }
                if self.pilot_mode {
                    self.rotate_ship_from_input(-1.0, 0.0);
                }
            }
            KeyCode::ArrowUp if self.interior_mode || self.pilot_mode => {
                self.look_up = true;
                if self.interior_mode {
                    self.rotate_player(0.0, -4.0);
                }
                if self.pilot_mode {
                    self.rotate_ship_from_input(0.0, -1.0);
                }
            }
            KeyCode::ArrowDown if self.interior_mode || self.pilot_mode => {
                self.look_down = true;
                if self.interior_mode {
                    self.rotate_player(0.0, 4.0);
                }
                if self.pilot_mode {
                    self.rotate_ship_from_input(0.0, 1.0);
                }
            }
            KeyCode::ArrowLeft => self
                .observation
                .adjust_orbit(-OBSERVATION_AZIMUTH_STEP, 0.0),
            KeyCode::ArrowRight => self.observation.adjust_orbit(OBSERVATION_AZIMUTH_STEP, 0.0),
            KeyCode::ArrowUp => self
                .observation
                .adjust_orbit(0.0, OBSERVATION_ELEVATION_STEP),
            KeyCode::ArrowDown => self
                .observation
                .adjust_orbit(0.0, -OBSERVATION_ELEVATION_STEP),
            _ => {}
        }
    }

    fn start_presented_mission(&mut self) {
        let mission = game::render_bridge::presentation_mission(self.session.world());
        let Some(mission) = mission else {
            self.feedback = "NO HAY MISION DISPONIBLE".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        };
        let Some(progress) = self.session.mission_progress(mission) else {
            self.feedback = "MISION NO DISPONIBLE".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        };
        if self.pilot_mode && progress.state != gameplay::domains::mission::MissionState::Active {
            self.exit_pilot();
        }
        let mut aggregate = FeedbackAggregate::default();
        // Preserve the existing M1 shortcut: the first mission is initially
        // locked in a new session, so the visual bootstrap unlocks it before
        // issuing the authoritative StartMission command. Later missions must
        // already be Available and are never unlocked by the visual layer.
        if progress.state == gameplay::domains::mission::MissionState::Locked
            && mission == game::campaign::CAMPAIGN_ORDER[0]
        {
            self.issue_into([GameplayCommand::UnlockMission { mission }], &mut aggregate);
        }
        if self
            .session
            .mission_progress(mission)
            .is_some_and(|value| value.state == gameplay::domains::mission::MissionState::Available)
        {
            if mission == crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA
                && !self.mission_two_destinations_compatible()
            {
                self.feedback = "MISSION 2 NO DISPONIBLE: DESTINO INCOMPATIBLE".to_owned();
                println!("visual gameplay: {}", self.feedback);
                return;
            }
            if mission == crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL
                && !self.mission_three_content_compatible()
            {
                self.feedback = "MISSION 3 NO DISPONIBLE: CONTENIDO INCOMPATIBLE".to_owned();
                println!("visual gameplay: {}", self.feedback);
                return;
            }
            let started =
                self.issue_into([GameplayCommand::StartMission { mission }], &mut aggregate);
            if started && mission == crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA {
                if !self.prepare_mission_two(&mut aggregate) {
                    self.feedback = "MISSION 2 PREPARACION FALLIDA".to_owned();
                    println!("visual gameplay: {}", self.feedback);
                    return;
                }
            }
            if started && mission == crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL {
                if !self.prepare_mission_three(&mut aggregate) {
                    self.feedback = "MISSION 3 PREPARACION FALLIDA".to_owned();
                    println!("visual gameplay: {}", self.feedback);
                    return;
                }
            }
            if started && mission == crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE {
                if !self.prepare_mission_four(&mut aggregate) {
                    self.feedback = "MISSION 4 PREPARACION FALLIDA".to_owned();
                    println!("visual gameplay: {}", self.feedback);
                    return;
                }
            }
            if started && mission == crate::cooperacion::PROGRESION_COOPERATIVA {
                if !self.prepare_mission_five_visual() {
                    self.feedback = "MISSION 5 PREPARACION FALLIDA".to_owned();
                    println!("visual gameplay: {}", self.feedback);
                    return;
                }
            }
        } else if self
            .session
            .mission_progress(mission)
            .is_some_and(|value| value.state == gameplay::domains::mission::MissionState::Active)
        {
            self.feedback = "MISION YA ACTIVA".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        } else if self
            .session
            .mission_progress(mission)
            .is_some_and(|value| value.state == gameplay::domains::mission::MissionState::Completed)
        {
            self.feedback = "MISION YA COMPLETADA".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        } else {
            self.feedback = "MISION NO DISPONIBLE".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        }
        self.finish_issue(aggregate);
    }

    fn mission_two_destinations() -> [Destination; 2] {
        [
            Destination {
                id: crate::ecos_anomalia::PLANET_DESTINATION,
                destination_type: DestinationType::Planet,
                position: GameplayVec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0,
                },
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Known,
            },
            Destination {
                id: crate::ecos_anomalia::SIGNAL_DESTINATION,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3 {
                    x: -35.0,
                    y: 8.0,
                    z: -20.0,
                },
                travel_duration: Duration(1_000),
                fuel_cost: 1,
                discovery: DiscoveryState::Unknown,
            },
        ]
    }

    fn destination_is_compatible(existing: &Destination, expected: &Destination) -> bool {
        let discovery_compatible = match expected.discovery {
            DiscoveryState::Unknown => true,
            DiscoveryState::Known => !matches!(existing.discovery, DiscoveryState::Unknown),
            DiscoveryState::Discovered => existing.discovery == DiscoveryState::Discovered,
        };
        existing.id == expected.id
            && existing.destination_type == expected.destination_type
            && existing.position == expected.position
            && existing.travel_duration == expected.travel_duration
            && existing.fuel_cost == expected.fuel_cost
            && discovery_compatible
    }

    fn mission_two_destinations_compatible(&self) -> bool {
        Self::mission_two_destinations().iter().all(|expected| {
            self.session
                .world()
                .destination(expected.id)
                .is_none_or(|existing| Self::destination_is_compatible(existing, expected))
        })
    }

    fn prepare_mission_two(&mut self, aggregate: &mut FeedbackAggregate) -> bool {
        let mut prepared = true;
        for destination in Self::mission_two_destinations() {
            if let Some(existing) = self.session.world().destination(destination.id) {
                if !Self::destination_is_compatible(existing, &destination) {
                    prepared = false;
                }
                continue;
            }
            if !self.issue_into(
                [GameplayCommand::RegisterDestination { destination }],
                aggregate,
            ) {
                prepared = false;
            }
        }
        if !prepared {
            return false;
        }
        if let Some(enemy) = self
            .session
            .world()
            .enemies()
            .find(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shooter)
                    && !value.is_defeated()
            })
            .map(|value| value.entity)
        {
            self.enemy = enemy;
            return true;
        }

        if !self.issue_into(
            [GameplayCommand::CreateEnemy {
                configuration: EnemyConfiguration::for_archetype(
                    EnemyArchetype::Shooter,
                    GameplayVec3 {
                        x: 10.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            }],
            aggregate,
        ) {
            return false;
        }
        if let Some(enemy) = self
            .session
            .world()
            .enemies()
            .find(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shooter)
                    && !value.is_defeated()
            })
            .map(|value| value.entity)
        {
            self.enemy = enemy;
        }
        self.session.world().enemies().any(|value| {
            value.active && value.archetype == Some(EnemyArchetype::Shooter) && !value.is_defeated()
        })
    }

    fn mission_three_destination() -> Destination {
        Destination {
            id: crate::tormenta_gravitacional::ESCAPE_DESTINATION,
            destination_type: DestinationType::Planet,
            position: GameplayVec3 {
                x: 20.0,
                y: 0.0,
                z: 0.0,
            },
            travel_duration: Duration(100),
            fuel_cost: 1,
            discovery: DiscoveryState::Known,
        }
    }

    fn mission_three_hazard(&self) -> EnvironmentalHazard {
        EnvironmentalHazard::new(
            crate::tormenta_gravitacional::HAZARD,
            gameplay::EnvironmentalHazardType::GravitationalAnomaly,
            self.session
                .world()
                .ship_position()
                .unwrap_or(GameplayVec3::ZERO),
            5.0,
            80,
            Duration(1_000),
            true,
        )
    }

    fn prepare_mission_three(&mut self, aggregate: &mut FeedbackAggregate) -> bool {
        let destination = Self::mission_three_destination();
        if let Some(existing) = self.session.world().destination(destination.id) {
            if !Self::destination_is_compatible(existing, &destination) {
                return false;
            }
        } else if !self.issue_into(
            [GameplayCommand::RegisterDestination { destination }],
            aggregate,
        ) {
            return false;
        }

        let hazard = self.mission_three_hazard();
        if let Some(existing) = self.session.world().environmental_hazard(hazard.id) {
            if existing != hazard {
                return false;
            }
        } else if !self.issue_into(
            [GameplayCommand::RegisterEnvironmentalHazard { hazard }],
            aggregate,
        ) {
            return false;
        }

        let has_material = self
            .session
            .world()
            .inventory(self.player)
            .is_some_and(|inventory| inventory.quantity(ItemId(1)) > 0)
            || self
                .session
                .world()
                .collectibles()
                .any(|value| value.active && value.item == ItemId(1));
        if !has_material {
            let position = self
                .session
                .world()
                .player(self.player)
                .map(|player| player.position)
                .unwrap_or(GameplayVec3::ZERO);
            if !self.issue_into(
                [GameplayCommand::CreateCollectible {
                    item: ItemId(1),
                    amount: 1,
                    position,
                    interaction_range: 3.0,
                }],
                aggregate,
            ) {
                return false;
            }
        }
        true
    }

    fn prepare_mission_four(&mut self, aggregate: &mut FeedbackAggregate) -> bool {
        if let Some(enemy) = self
            .session
            .world()
            .enemies()
            .find(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shielded)
                    && !value.is_defeated()
            })
            .map(|value| value.entity)
        {
            self.enemy = enemy;
            return true;
        }
        let player_position = self
            .session
            .world()
            .player(self.player)
            .map(|player| player.position)
            .unwrap_or(GameplayVec3::ZERO);
        let position = GameplayVec3 {
            x: player_position.x + 4.0,
            y: player_position.y,
            z: player_position.z,
        };
        if !self.issue_into(
            [GameplayCommand::CreateEnemy {
                configuration: EnemyConfiguration::for_archetype(
                    EnemyArchetype::Shielded,
                    position,
                ),
            }],
            aggregate,
        ) {
            return false;
        }
        if let Some(enemy) = self
            .session
            .world()
            .enemies()
            .find(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shielded)
                    && !value.is_defeated()
            })
            .map(|value| value.entity)
        {
            self.enemy = enemy;
            true
        } else {
            false
        }
    }

    fn prepare_mission_five_visual(&mut self) -> bool {
        if self
            .session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_none()
        {
            return false;
        }
        if let Some(enemy) = self
            .session
            .world()
            .enemies()
            .find(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shooter)
                    && !value.is_defeated()
            })
            .map(|value| value.entity)
        {
            self.enemy = enemy;
            true
        } else {
            false
        }
    }

    fn equip_energy_weapon(&mut self) {
        let completed = self.issue([GameplayCommand::EquipPlayerWeapon {
            player: self.player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Energy,
                100,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ItemId(3),
                    amount_per_attack: 1,
                },
            ),
        }]);
        if completed {
            self.feedback = "ARMA DE ENERGIA EQUIPADA".to_owned();
            println!("visual gameplay: {}", self.feedback);
        }
    }

    fn equip_found_weapon(&mut self, item: ItemId) {
        if self.issue([GameplayCommand::EquipPlayerWeaponItem {
            player: self.player,
            item,
        }]) {
            self.feedback = match item {
                gameplay::ENERGY_WEAPON_ITEM => "ARMA DE ENERGIA EQUIPADA".to_owned(),
                gameplay::PLASMA_WEAPON_ITEM => "ARMA DE PLASMA EQUIPADA".to_owned(),
                _ => "ARMA EQUIPADA".to_owned(),
            };
            println!("visual gameplay: {}", self.feedback);
        }
    }

    fn equip_kinetic_weapon(&mut self) {
        let completed = self.issue([GameplayCommand::EquipPlayerWeapon {
            player: self.player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                20,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 1,
                },
            ),
        }]);
        if completed {
            self.feedback = "ARMA CINETICA EQUIPADA".to_owned();
            println!("visual gameplay: {}", self.feedback);
        }
    }

    fn mission_three_content_compatible(&self) -> bool {
        let destination = Self::mission_three_destination();
        let destination_compatible = self
            .session
            .world()
            .destination(destination.id)
            .is_none_or(|existing| Self::destination_is_compatible(existing, &destination));
        let hazard = self.mission_three_hazard();
        let hazard_compatible = self
            .session
            .world()
            .environmental_hazard(hazard.id)
            .is_none_or(|existing| existing == hazard);
        destination_compatible && hazard_compatible
    }

    fn retry_cooperative_mission_content(&mut self) {
        let results = self
            .session
            .update([GameplayCommand::RetryCooperativeMissionContent]);
        self.refresh_player_defeat_state();
        let status = results.first().map(|result| &result.status);
        self.feedback = match status {
            Some(gameplay::CommandStatus::Success { .. }) => {
                "MISSION 5 PREPARADA — PUEDES CONTINUAR".to_owned()
            }
            Some(gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::MissionContentUnavailable,
            }) => "CONTENIDO DE MISSION 5 NO DISPONIBLE — PUEDES REINTENTAR".to_owned(),
            Some(gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::InvalidMissionTransition,
            }) => "RECUPERACION NO DISPONIBLE — MISSION 5 NO ESTA ACTIVA".to_owned(),
            Some(other) => format!("RECUPERACION DE MISSION 5 RECHAZADA: {other:?}"),
            None => "RECUPERACION DE MISSION 5 SIN RESULTADO".to_owned(),
        };
        println!("visual gameplay: {}", self.feedback);
    }

    fn guided_interaction(&mut self) {
        let at_pilot_station = self.interior_mode
            && self.mission_one_completed()
            && !self.legacy_exterior_controls
            && self
                .session
                .world()
                .player(self.player)
                .zip(self.session.world().interactable(self.pilot_station))
                .is_some_and(|(player, station)| {
                    let dx = player.position.x - station.position.x;
                    let dy = player.position.y - station.position.y;
                    let dz = player.position.z - station.position.z;
                    dx * dx + dy * dy + dz * dz
                        <= station.interaction_range * station.interaction_range
                });
        if at_pilot_station {
            if self.pilot_station_occupied_by(self.player)
                || self.issue([GameplayCommand::Interact {
                    player: self.player,
                    target: self.pilot_station,
                }])
            {
                self.enter_pilot();
            }
            return;
        }
        let can_access_pilot_console = !self.pilot_mode
            && !self.legacy_exterior_controls
            && !self.interior_mode
            && self.mission_one_completed();
        if can_access_pilot_console {
            let mut aggregate = FeedbackAggregate::default();
            self.issue_into(
                [GameplayCommand::ReleaseStation {
                    station: self.pilot_station,
                    player: self.player,
                }],
                &mut aggregate,
            );
            let occupied = self.issue_into(
                [GameplayCommand::Interact {
                    player: self.player,
                    target: self.pilot_station,
                }],
                &mut aggregate,
            );
            if occupied {
                self.finish_issue(aggregate);
                self.enter_pilot();
            } else {
                self.feedback = aggregate.message();
            }
            return;
        }
        if let Some(derelict) = self.selected_derelict().cloned() {
            let mut commands = Vec::new();
            if derelict.state == gameplay::DerelictState::Undiscovered {
                commands.push(GameplayCommand::DiscoverDerelict {
                    player: self.player,
                    derelict: derelict.id,
                });
            }
            if derelict.state != gameplay::DerelictState::Resolved {
                commands.push(GameplayCommand::ExploreDerelict {
                    player: self.player,
                    derelict: derelict.id,
                });
            }
            if self.issue(commands) {
                self.feedback = "NAVE ABANDONADA EXPLORADA".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
            return;
        }
        if let Some(collectible) = self.selected_weapon_collectible() {
            if self.issue([GameplayCommand::CollectItem {
                player: self.player,
                collectible: collectible.entity,
                amount: collectible.amount,
            }]) {
                self.feedback = "ARMAMENTO RECUPERADO - USA B PARA EQUIPAR PLASMA".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
            return;
        }
        if let Some(collectible) = self.selected_resource_collectible() {
            if self.issue([GameplayCommand::CollectItem {
                player: self.player,
                collectible: collectible.entity,
                amount: collectible.amount,
            }]) {
                self.feedback = "RECURSO RECUPERADO".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
            return;
        }
        let action = self
            .session
            .world()
            .guidance_for_player(self.player)
            .ok()
            .and_then(|guidance| guidance.recommended_action);
        let pilot_console_available = action.is_none()
            && !self.pilot_mode
            && !self.legacy_exterior_controls
            && !self.interior_mode
            && self.mission_one_completed();
        if (self.interior_mode || pilot_console_available) && action.is_none() {
            let mut aggregate = FeedbackAggregate::default();
            self.issue_into(
                [GameplayCommand::ReleaseStation {
                    station: self.pilot_station,
                    player: self.player,
                }],
                &mut aggregate,
            );
            let occupied = self.issue_into(
                [GameplayCommand::Interact {
                    player: self.player,
                    target: self.pilot_station,
                }],
                &mut aggregate,
            );
            if occupied {
                self.finish_issue(aggregate);
                self.enter_pilot();
            } else {
                self.feedback = aggregate.message();
            }
            return;
        }
        if matches!(action, Some(gameplay::GuidanceAction::RepairShip)) {
            let mut aggregate = FeedbackAggregate::default();
            let occupied = self.issue_into(
                [GameplayCommand::Interact {
                    player: self.player,
                    target: self.engineering_station,
                }],
                &mut aggregate,
            );
            if occupied {
                self.issue_into(
                    [GameplayCommand::RepairShipAtStation {
                        player: self.player,
                        ship: self.ship,
                        station: self.engineering_station,
                    }],
                    &mut aggregate,
                );
            }
            self.finish_issue(aggregate);
            return;
        }
        if let Some(gameplay::GuidanceAction::Interact(target)) = action {
            let mut aggregate = FeedbackAggregate::default();
            let pilot_access_requested = target == self.pilot_station
                && !self.legacy_exterior_controls
                && self.mission_one_completed();
            let behavior = self
                .session
                .world()
                .interactable(target)
                .map(|value| value.behavior);
            if target == self.bridge_door {
                self.issue_into(
                    [GameplayCommand::UnlockDoor {
                        door: self.bridge_door,
                    }],
                    &mut aggregate,
                );
            }
            if target == self.pilot_station && self.pilot_station_occupied_by(self.player) {
                self.issue_into(
                    [GameplayCommand::ReleaseStation {
                        station: self.pilot_station,
                        player: self.player,
                    }],
                    &mut aggregate,
                );
            }
            let interaction_accepted = self.issue_into(
                [GameplayCommand::Interact {
                    player: self.player,
                    target,
                }],
                &mut aggregate,
            );

            // Panel interactions authorize the corresponding gameplay systems;
            // the follow-up commands still perform all normal range,
            // configuration and dependency validation in gameplay.
            if interaction_accepted {
                match behavior {
                    Some(InteractionBehavior::ElectricalPanel) => {
                        self.issue_into(
                            [GameplayCommand::SetElectricalBreakerState {
                                player: self.player,
                                panel: target,
                                breaker: BreakerId::new(1),
                                state: BreakerState::On,
                            }],
                            &mut aggregate,
                        );
                    }
                    Some(InteractionBehavior::LifeSupportPanel) => {
                        self.issue_into(
                            [GameplayCommand::SetOxygenAllocation {
                                player: self.player,
                                panel: target,
                                compartment: gameplay::CompartmentId::new(1),
                                allocation: 3,
                            }],
                            &mut aggregate,
                        );
                        self.issue_into(
                            [GameplayCommand::SetOxygenAllocation {
                                player: self.player,
                                panel: target,
                                compartment: gameplay::CompartmentId::new(2),
                                allocation: 2,
                            }],
                            &mut aggregate,
                        );
                    }
                    _ => {}
                }
            }
            let completed = self.finish_issue(aggregate);
            if completed && pilot_access_requested {
                self.enter_pilot();
                return;
            }
            if completed
                && target == self.bridge_door
                && self.interior_mode
                && self
                    .session
                    .world()
                    .ship_station_anchor(self.pilot_station)
                    .is_none()
            {
                self.interior_mode = false;
                self.feedback = "PUERTA ABIERTA — EXPLORACION EXTERIOR DISPONIBLE".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
        } else {
            let target = self.selected_interactable().map(|value| value.entity_id);
            if let Some(target) = target {
                let completed = self.issue([GameplayCommand::Interact {
                    player: self.player,
                    target,
                }]);
                if completed
                    && target == self.bridge_door
                    && self.interior_mode
                    && self
                        .session
                        .world()
                        .ship_station_anchor(self.pilot_station)
                        .is_none()
                {
                    self.interior_mode = false;
                    self.feedback = "PUERTA ABIERTA — EXPLORACION EXTERIOR DISPONIBLE".to_owned();
                    println!("visual gameplay: {}", self.feedback);
                }
            } else {
                self.feedback = "NO HAY INTERACCION DISPONIBLE — ACERCATE A UN OBJETO".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
        }
    }

    fn rotate_player(&mut self, yaw_delta_degrees: f32, pitch_delta_degrees: f32) {
        self.issue([GameplayCommand::RotatePlayer {
            player: self.player,
            yaw_delta_degrees,
            pitch_delta_degrees,
        }]);
    }

    fn sync_ship_interior(&mut self) {
        let Some(origin) = self.session.world().ship_position() else {
            return;
        };
        if origin == self.interior_anchor {
            return;
        }
        let placements = [
            (self.pilot_station, GameplayVec3 { x: 0.0, y: 0.0, z: 1.5 }),
            (self.engineering_station, GameplayVec3 { x: 2.5, y: 0.0, z: 0.0 }),
            (self.status_console, GameplayVec3 { x: -2.0, y: 0.0, z: 0.0 }),
            (self.electrical_panel, GameplayVec3 { x: 0.0, y: 0.0, z: -2.0 }),
            (self.life_support_panel, GameplayVec3 { x: 2.0, y: 0.0, z: 0.0 }),
            (self.bridge_door, GameplayVec3 { x: 0.0, y: 0.0, z: 2.0 }),
        ];
        for (entity, local) in placements {
            let position = GameplayVec3 {
                x: origin.x + local.x,
                y: origin.y + local.y,
                z: origin.z + local.z,
            };
            let _ = self
                .session
                .world_mut()
                .set_interactable_position(entity, position);
        }
        self.interior_anchor = origin;
    }

    fn update_interior_motion(&mut self) {
        if (!self.interior_mode && !self.surface_mode) || self.player_defeated {
            return;
        }
        if self.interior_mode {
            self.sync_ship_interior();
        }
        let Some(player) = self.session.world().player(self.player).copied() else {
            return;
        };
        let now = Instant::now();
        let delta_ms = now
            .saturating_duration_since(self.last_interior_update)
            .as_millis()
            .clamp(1, 50) as u64;
        self.last_interior_update = now;
        let seconds = delta_ms as f32 / 1_000.0;
        let local = self.current_flight_input();
        let interior_origin = self
            .session
            .world()
            .ship_position()
            .unwrap_or(GameplayVec3::ZERO);
        let yaw = player.orientation.yaw_degrees.to_radians();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let mut direction = GameplayVec3 {
            x: local.x * cos_yaw - local.z * sin_yaw,
            y: 0.0,
            z: local.x * sin_yaw + local.z * cos_yaw,
        };
        let look_yaw = self.look_right as i8 - self.look_left as i8;
        let look_pitch = self.look_down as i8 - self.look_up as i8;
        if look_yaw != 0 || look_pitch != 0 {
            self.rotate_player(
                f32::from(look_yaw) * 6.0 * seconds / 0.016,
                f32::from(look_pitch) * 4.0 * seconds / 0.016,
            );
        }
        let horizontal_magnitude = (direction.x * direction.x + direction.z * direction.z).sqrt();
        if horizontal_magnitude > 1.0 {
            direction.x /= horizontal_magnitude;
            direction.z /= horizontal_magnitude;
        }
        direction.y = 0.0;
        if direction != GameplayVec3::ZERO {
            self.session.update([GameplayCommand::MovePlayer {
                player: self.player,
                direction,
                delta: Duration(delta_ms),
            }]);
        } else if player.velocity != GameplayVec3::ZERO {
            self.session.update([GameplayCommand::MovePlayer {
                player: self.player,
                direction: GameplayVec3::ZERO,
                delta: Duration(delta_ms),
            }]);
        }
        if let Some(updated) = self.session.world().player(self.player).copied() {
            let ship_yaw = self
                .session
                .world()
                .ship_orientation()
                .map(|orientation| orientation.yaw_degrees)
                .unwrap_or_default();
            let current_surface = game::render_bridge::echo7_support_surface(
                player.position,
                interior_origin,
                ship_yaw,
            );
            let door = self.session.world().interactable(self.bridge_door);
            let door_position = door.map(|value| value.position);
            let door_open = door.is_some_and(|value| {
                matches!(
                    value.behavior,
                    gameplay::InteractionBehavior::Door { door }
                        if matches!(door.state, gameplay::domains::door::DoorState::Open)
                )
            });
            let room_open: [bool; game::render_bridge::ECHO7_ROOM_DOORS.len()] =
                std::array::from_fn(|index| {
                    self.room_doors.get(index).is_some_and(|entity| {
                        self.session.world().interactable(*entity).is_some_and(|value| {
                            matches!(
                                value.behavior,
                                gameplay::InteractionBehavior::Door { door }
                                    if matches!(door.state, gameplay::domains::door::DoorState::Open)
                            )
                        })
                    })
                });
            let lower_deck_path_valid = if matches!(
                current_surface,
                game::render_bridge::Echo7SupportSurface::LowerDeck
            ) {
                let distance = ((updated.position.x - player.position.x).powi(2)
                    + (updated.position.z - player.position.z).powi(2))
                .sqrt();
                let samples = (distance / 0.05).ceil().max(1.0) as u32;
                (1..=samples).all(|step| {
                    let fraction = step as f32 / samples as f32;
                    let candidate = GameplayVec3 {
                        x: player.position.x + (updated.position.x - player.position.x) * fraction,
                        y: player.position.y,
                        z: player.position.z + (updated.position.z - player.position.z) * fraction,
                    };
                    if self.room_doors.is_empty() {
                        game::render_bridge::echo7_lower_deck_position_valid(
                            candidate,
                            interior_origin,
                            ship_yaw,
                            door_position,
                            door_open,
                        )
                    } else {
                        game::render_bridge::echo7_model_floor_position_valid(
                            candidate,
                            interior_origin,
                            ship_yaw,
                            false,
                            &room_open,
                        )
                    }
                })
            } else {
                true
            };
            let upper_deck_path_valid = if !self.room_doors.is_empty()
                && matches!(
                    current_surface,
                    game::render_bridge::Echo7SupportSurface::UpperBalcony
                ) {
                let distance = ((updated.position.x - player.position.x).powi(2)
                    + (updated.position.z - player.position.z).powi(2))
                .sqrt();
                let samples = (distance / 0.05).ceil().max(1.0) as u32;
                (1..=samples).all(|step| {
                    let fraction = step as f32 / samples as f32;
                    game::render_bridge::echo7_model_floor_position_valid(
                        GameplayVec3 {
                            x: player.position.x
                                + (updated.position.x - player.position.x) * fraction,
                            y: player.position.y,
                            z: player.position.z
                                + (updated.position.z - player.position.z) * fraction,
                        },
                        interior_origin,
                        ship_yaw,
                        true,
                        &room_open,
                    )
                })
            } else {
                true
            };
            let stair_path_valid = if matches!(
                current_surface,
                game::render_bridge::Echo7SupportSurface::Stair
            ) {
                let horizontal_distance = ((updated.position.x - player.position.x).powi(2)
                    + (updated.position.z - player.position.z).powi(2))
                .sqrt();
                let sample_count = (horizontal_distance / 0.05).ceil().max(1.0) as u32;
                (1..=sample_count).all(|step| {
                    let fraction = step as f32 / sample_count as f32;
                    let candidate = GameplayVec3 {
                        x: player.position.x + (updated.position.x - player.position.x) * fraction,
                        y: player.position.y + (updated.position.y - player.position.y) * fraction,
                        z: player.position.z + (updated.position.z - player.position.z) * fraction,
                    };
                    game::render_bridge::echo7_stair_path_position_valid(
                        engine::Vec3 {
                            x: candidate.x,
                            y: candidate.y,
                            z: candidate.z,
                        },
                        interior_origin,
                        ship_yaw,
                    )
                })
            } else {
                true
            };
            let support = game::render_bridge::echo7_walkable_height(
                updated.position,
                interior_origin,
                ship_yaw,
            );
            let crossed_upper_edge = player.position.y > 1.0
                && !game::render_bridge::echo7_upper_or_stair_surface_contains(
                    updated.position,
                    interior_origin,
                    ship_yaw,
                );
            if !lower_deck_path_valid
                || !upper_deck_path_valid
                || !stair_path_valid
                || crossed_upper_edge
            {
                let mut reverted_position = updated.position;
                reverted_position.x = player.position.x;
                reverted_position.z = player.position.z;
                reverted_position.y = player.position.y;
                self.session
                    .world_mut()
                    .initialize_player_position(self.player, reverted_position);
            } else if (support - updated.position.y).abs() <= 0.75 {
                let mut supported_position = updated.position;
                supported_position.y = support;
                self.session
                    .world_mut()
                    .initialize_player_position(self.player, supported_position);
            } else if player.position.y > 1.0 && support < 1.0 {
                let mut reverted_position = updated.position;
                reverted_position.x = player.position.x;
                reverted_position.z = player.position.z;
                reverted_position.y = player.position.y;
                self.session
                    .world_mut()
                    .initialize_player_position(self.player, reverted_position);
            }
            if matches!(
                std::env::var("ASTRAL_ECHO_STAIR_DIAGNOSTIC").as_deref(),
                Ok("1") | Ok("true")
            ) {
                self.feedback = format!(
                    "STAIR feet=({:.2},{:.2},{:.2}) support={:.2} balcony=ring[r=8.45..10.55]+connection",
                    updated.position.x,
                    updated.position.y,
                    updated.position.z,
                    support
                );
            }
        }
    }

    fn contextual_prompt(&self) -> Option<String> {
        self.contextual_action()
            .map(|action| format!("{} [{}] {}", action.object, action.key, action.action))
    }

    fn contextual_action(&self) -> Option<render_core::HudContextualAction> {
        if self.surface_mode {
            if let Some(planet) = self.surface_planet {
                if let (Some(player), Some(ship_position)) = (
                    self.session.world().player(self.player),
                    game::render_bridge::surface_ship_position(self.session.world(), planet),
                ) {
                    let dx = player.position.x - ship_position.x;
                    let dy = player.position.y - ship_position.y;
                    let dz = player.position.z - ship_position.z;
                    if (dx * dx + dy * dy + dz * dz).sqrt()
                        <= game::render_bridge::SURFACE_RETURN_DISTANCE
                    {
                        return Some(render_core::HudContextualAction {
                            target: game::render_bridge::LANDING_INDICATOR_ENTITY.value(),
                            object: "ECHO-7".to_owned(),
                            action: "DESPEGAR".to_owned(),
                            key: "I".to_owned(),
                        });
                    }
                }
            }
        }
        if let Some(planet) = self.landing_target() {
            return Some(render_core::HudContextualAction {
                target: planet.value() as u64,
                object: Self::navigation_destination_label(planet).to_owned(),
                action: "ATERRIZAR".to_owned(),
                key: "L".to_owned(),
            });
        }
        if let Some(interactable) = self.selected_interactable() {
            let (object, action) = match interactable.behavior {
                InteractionBehavior::Door { .. } => ("PUERTA DEL PUENTE", "ABRIR"),
                InteractionBehavior::Station { station } => match station.station_type {
                    StationType::Piloting => ("CONSOLA DE PILOTAJE", "ACCEDER"),
                    StationType::Navigation => ("ESTACION DE NAVEGACION", "ACCEDER"),
                    StationType::Engineering => ("ESTACION DE INGENIERIA", "ACCEDER"),
                    StationType::LifeSupport => ("ESTACION DE SOPORTE VITAL", "ACCEDER"),
                    StationType::Weapons => ("ESTACION DE ARMAS", "ACCEDER"),
                },
                InteractionBehavior::ElectricalPanel => ("PANEL ELECTRICO", "REPARAR"),
                InteractionBehavior::LifeSupportPanel => ("SOPORTE VITAL", "REPARAR"),
                InteractionBehavior::Console { role, .. } => match role {
                    gameplay::ConsoleRole::ShipStatus => ("CONSOLA DE ESTADO", "CONSULTAR"),
                    gameplay::ConsoleRole::Generic => ("CONSOLA", "INTERACTUAR"),
                },
            };
            return Some(render_core::HudContextualAction {
                target: interactable.entity_id.value(),
                object: object.to_owned(),
                action: action.to_owned(),
                key: "I".to_owned(),
            });
        }
        if let Some(derelict) = self.selected_derelict() {
            return Some(render_core::HudContextualAction {
                target: u64::from(derelict.id.value()),
                object: "NAVE ABANDONADA".to_owned(),
                action: match derelict.state {
                    gameplay::DerelictState::Undiscovered => "DESCUBRIR",
                    gameplay::DerelictState::Discovered => "EXPLORAR",
                    _ => "REVISAR",
                }
                .to_owned(),
                key: "I".to_owned(),
            });
        }
        self.selected_weapon_collectible()
            .map(|collectible| render_core::HudContextualAction {
                target: collectible.entity.value(),
                object: match collectible.item {
                    gameplay::ENERGY_WEAPON_ITEM => "ARMA DE ENERGIA",
                    gameplay::PLASMA_WEAPON_ITEM => "ARMA DE PLASMA",
                    _ => "ARMAMENTO",
                }
                .to_owned(),
                action: "RECOGER".to_owned(),
                key: "I".to_owned(),
            })
            .or_else(|| {
                self.selected_resource_collectible().map(|collectible| {
                    render_core::HudContextualAction {
                        target: collectible.entity.value(),
                        object: match collectible.item {
                            gameplay::FUEL_RESOURCE_ITEM => "COMBUSTIBLE",
                            gameplay::OXYGEN_RESOURCE_ITEM => "OXIGENO",
                            _ => "SUMINISTROS",
                        }
                        .to_owned(),
                        action: "RECOGER".to_owned(),
                        key: "I".to_owned(),
                    }
                })
            })
    }

    fn selected_derelict(&self) -> Option<&gameplay::Derelict> {
        if self.interior_mode || self.pilot_mode {
            return None;
        }
        let position = self.session.world().ship_position()?;
        self.session
            .world()
            .derelicts()
            .filter(|value| {
                matches!(
                    value.state,
                    gameplay::DerelictState::Undiscovered | gameplay::DerelictState::Discovered
                )
            })
            .filter_map(|value| {
                let dx = position.x - value.position.x;
                let dy = position.y - value.position.y;
                let dz = position.z - value.position.z;
                let distance_squared = dx * dx + dy * dy + dz * dz;
                (distance_squared <= value.interaction_range * value.interaction_range)
                    .then_some((distance_squared, value))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.id.cmp(&right.1.id))
            })
            .map(|(_, value)| value)
    }

    fn selected_weapon_collectible(&self) -> Option<gameplay::Collectible> {
        if self.interior_mode || self.pilot_mode {
            return None;
        }
        let position = self.session.world().ship_position()?;
        self.session
            .world()
            .collectibles()
            .filter(|value| {
                value.active
                    && matches!(
                        value.item,
                        gameplay::ENERGY_WEAPON_ITEM | gameplay::PLASMA_WEAPON_ITEM
                    )
            })
            .filter_map(|value| {
                let dx = position.x - value.position.x;
                let dy = position.y - value.position.y;
                let dz = position.z - value.position.z;
                let distance_squared = dx * dx + dy * dy + dz * dz;
                (distance_squared <= value.interaction_range * value.interaction_range)
                    .then_some((distance_squared, *value))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.entity.cmp(&right.1.entity))
            })
            .map(|(_, value)| value)
    }

    fn selected_resource_collectible(&self) -> Option<gameplay::Collectible> {
        if self.interior_mode || self.pilot_mode {
            return None;
        }
        let position = self.session.world().ship_position()?;
        self.session
            .world()
            .collectibles()
            .filter(|value| {
                value.active
                    && matches!(
                        self.session
                            .world()
                            .item_definition(value.item)
                            .map(|definition| definition.category),
                        Some(gameplay::ItemCategory::Fuel)
                            | Some(gameplay::ItemCategory::Oxygen)
                            | Some(gameplay::ItemCategory::RepairMaterial)
                    )
            })
            .filter_map(|value| {
                let dx = position.x - value.position.x;
                let dy = position.y - value.position.y;
                let dz = position.z - value.position.z;
                let distance_squared = dx * dx + dy * dy + dz * dz;
                (distance_squared <= value.interaction_range * value.interaction_range)
                    .then_some((distance_squared, *value))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.entity.cmp(&right.1.entity))
            })
            .map(|(_, value)| value)
    }

    fn selected_interactable(&self) -> Option<&gameplay::Interactable> {
        if !self.interior_mode || self.pilot_mode {
            return None;
        }
        let player = self.session.world().player(self.player)?;
        self.session
            .world()
            .interactables()
            .filter(|value| {
                value.enabled
                    && match value.behavior {
                        InteractionBehavior::Station { station } => {
                            station.occupant.is_none() || station.occupant == Some(self.player)
                        }
                        _ => true,
                    }
            })
            .filter_map(|value| {
                let dx = player.position.x - value.position.x;
                let dy = player.position.y - value.position.y;
                let dz = player.position.z - value.position.z;
                let distance_squared = dx * dx + dy * dy + dz * dz;
                (distance_squared <= value.interaction_range * value.interaction_range)
                    .then_some((distance_squared, value))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.entity_id.cmp(&right.1.entity_id))
            })
            .map(|(_, interactable)| interactable)
    }

    fn instructions_hint(&self) -> String {
        if !self.started {
            return "PULSA ENTER PARA COMENZAR".to_owned();
        }
        if self.pilot_mode {
            if self.navigation_selector_open {
                let candidate = self
                    .free_navigation_candidate
                    .map(Self::navigation_destination_label)
                    .unwrap_or("NINGUNO");
                return format!(
                    "SELECCIÓN DE DESTINO | CANDIDATO: {candidate} | FLECHAS CAMBIAR | ENTER CONFIRMAR | N CERRAR"
                );
            }
            let confirmed = self
                .session
                .world()
                .free_navigation_destination()
                .map(Self::navigation_destination_label)
                .unwrap_or("NINGUNO");
            let proposal = self.free_navigation_proposal_status();
            let autopilot = match self.session.world().autopilot_state() {
                gameplay::AutopilotState::InTransit { .. }
                    if matches!(
                        self.session.world().travel_state(),
                        gameplay::TravelState::InProgress { .. }
                    ) =>
                {
                    "EN TRANSITO"
                }
                gameplay::AutopilotState::Completed { .. } => "COMPLETADO",
                gameplay::AutopilotState::Interrupted { .. } => "CANCELADO",
                _ => "INACTIVO",
            };
            return format!(
                "PILOTAJE ACTIVO: OBSERVANDO {} - DESTINO LIBRE: {confirmed} ({proposal}) - AUTOPILOTAJE: {autopilot} - N SELECCIONAR - [/] OBSERVAR - F1 CONTROLES - F2 HUD - F3 INVENTARIO",
                self.observation_target_label(),
            );
        }
        if let Some(prompt) = self.contextual_prompt() {
            return format!("{prompt} | PULSA LA TECLA INDICADA");
        }
        if self.surface_mode {
            return "EXPLORACION PLANETARIA: WASD MOVER | FLECHAS MIRAR | ACERCATE A ECHO-7 Y PULSA I PARA DESPEGAR".to_owned();
        }
        if self.interior_mode {
            if self
                .guidance_prompt()
                .is_some_and(|value| value.starts_with('M'))
            {
                return "PULSA M PARA INICIAR LA MISION DISPONIBLE".to_owned();
            }
            return "EXPLORACION: F1 CONTROLES - F2 HUD - F3 INVENTARIO".to_owned();
        }
        "EXTERIOR: SIGUE LA GUIA - F1 CONTROLES - F2 HUD - F3 INVENTARIO".to_owned()
    }

    fn guidance_prompt(&self) -> Option<String> {
        let guidance = self.session.world().guidance_for_player(self.player).ok()?;
        let objective = guidance
            .current_objective
            .and_then(|id| guidance.objectives.iter().find(|value| value.id == id));
        let Some(objective) = objective else {
            return self
                .session
                .world()
                .missions()
                .find(|mission| {
                    mission.state() == gameplay::domains::mission::MissionState::Available
                })
                .map(|_| "M INICIAR MISION DISPONIBLE".to_owned())
                .or_else(|| {
                    self.session
                        .mission_progress(game::campaign::CAMPAIGN_ORDER[0])
                        .is_some_and(|mission| {
                            mission.state == gameplay::domains::mission::MissionState::Locked
                        })
                        .then_some("M DESPERTAR Y COMENZAR MISION 1".to_owned())
                })
                .or_else(|| Some("SIN OBJETIVO ACTIVO".to_owned()));
        };
        if let Some(requirement) = objective.missing_requirements.first() {
            return Some(format!(
                "{} | {} | PENDIENTE: {}",
                objective.name,
                guidance_action_label(objective.recommended_action),
                guidance_requirement_label(*requirement),
            ));
        }
        Some(format!(
            "{} | {}",
            objective.name,
            guidance_action_label(objective.recommended_action)
        ))
    }

    fn controls_hint(&self) -> String {
        if !self.started {
            "ENTER COMENZAR".to_owned()
        } else if self.pilot_mode {
            let autopilot = match self.session.world().autopilot_state() {
                gameplay::AutopilotState::Inactive => "INACTIVO",
                gameplay::AutopilotState::Planned { .. } => "PLANIFICADO",
                gameplay::AutopilotState::InTransit { .. } => "EN TRANSITO",
                gameplay::AutopilotState::Completed { .. } => "COMPLETADO",
                gameplay::AutopilotState::Interrupted { .. } => "INTERRUMPIDO",
                gameplay::AutopilotState::Failed { .. } => "FALLIDO",
            };
            format!("MODO PILOTAJE ECHO-7 | W/S AVANCE/RETROCESO | A/D GIRAR | ESPACIO/CTRL ALTURA | PGUP/PGDN DISTANCIA CAMARA | P IR AL PLANETA DE LA GUIA | I SALIR | AUTO: {autopilot}")
        } else if self.surface_mode {
            "WASD MOVER | FLECHAS MIRAR | I DESPEGAR AL ACERCARTE A ECHO-7".to_owned()
        } else if self.interior_mode {
            "WASD MOVER | FLECHAS MIRAR | I USAR | M INICIAR".to_owned()
        } else if self
            .session
            .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
            .is_some_and(|mission| {
                mission.state == gameplay::domains::mission::MissionState::Active
            })
        {
            "Q ESCANEAR | P VIAJAR | F ATACAR | E RECOGER | T RECUPERAR".to_owned()
        } else {
            "Q ESCANEAR | P VIAJAR | F ATACAR | E RECOGER".to_owned()
        }
    }

    fn controls_panel_entries(&self) -> Vec<String> {
        let mut entries = vec![
            "F1 ABRIR O CERRAR PANEL".to_owned(),
            "F2 OCULTAR O MOSTRAR HUD".to_owned(),
            "F3 ABRIR O CERRAR INVENTARIO".to_owned(),
        ];
        if self.pilot_mode {
            entries.extend([
                "W AVANZAR | S RETROCEDER | A/D GIRAR - PILOTAJE".to_owned(),
                "ESPACIO SUBIR | CTRL BAJAR - PILOTAJE".to_owned(),
                "PGUP/PGDN ACERCAR/ALEJAR CAMARA - PILOTAJE".to_owned(),
                "F DISPARAR - PILOTAJE".to_owned(),
                "G ESCUDO - PILOTAJE".to_owned(),
                "SHIFT BOOST - PILOTAJE".to_owned(),
                "X FRENAR - PILOTAJE".to_owned(),
                "L ATERRIZAR CUANDO EL PLANETA ESTE CERCA - PILOTAJE".to_owned(),
                "[/] CAMBIAR PLANETA OBSERVADO - PILOTAJE".to_owned(),
                "N SELECCIONAR DESTINO LIBRE - PILOTAJE".to_owned(),
                "U USAR COMBUSTIBLE - PILOTAJE".to_owned(),
                "B EQUIPAR PLASMA - ARMAMENTO ENCONTRADO".to_owned(),
                "P IR AL PLANETA DE LA GUIA - PILOTAJE".to_owned(),
                "I SALIR DEL PILOTAJE - PILOTAJE".to_owned(),
            ]);
        } else if self.surface_mode {
            entries.extend([
                "WASD MOVER | FLECHAS MIRAR - EXPLORACION PLANETARIA".to_owned(),
                "I DESPEGAR AL ACERCARTE A ECHO-7".to_owned(),
            ]);
        } else if self.interior_mode {
            entries.extend([
                "WASD MOVER - EXPLORACION".to_owned(),
                "FLECHAS MIRAR - EXPLORACION".to_owned(),
                "I INTERACTUAR - OBJETOS".to_owned(),
                "M INICIAR MISION - GENERAL".to_owned(),
                "R RESTAURAR ENERGIA - PANEL ELECTRICO".to_owned(),
                "Y RESTAURAR SOPORTE VITAL - PANEL DE OXIGENO".to_owned(),
                "TAB CAMBIAR ARMA - INVENTARIO".to_owned(),
            ]);
        } else {
            entries.extend([
                "Q ESCANEAR - EXTERIOR".to_owned(),
                "P VIAJAR - EXTERIOR".to_owned(),
                "O OBSERVAR PLANETAS | [/] CAMBIAR OBJETIVO".to_owned(),
                "L ATERRIZAR AL ESTAR CERCA DE UN PLANETA".to_owned(),
                "F ATACAR - COMBATE".to_owned(),
                "E RECOGER - EXTERIOR".to_owned(),
                "TAB CAMBIAR ARMA - INVENTARIO".to_owned(),
            ]);
            if self
                .session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .is_some_and(|mission| {
                    mission.state == gameplay::domains::mission::MissionState::Active
                })
            {
                entries.push("T RECUPERAR - COOPERACION".to_owned());
            }
        }
        entries
    }

    fn restore_energy(&mut self) {
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into(
            [GameplayCommand::Interact {
                player: self.player,
                target: self.electrical_panel,
            }],
            &mut aggregate,
        );
        self.issue_into(
            [GameplayCommand::SetElectricalBreakerState {
                player: self.player,
                panel: self.electrical_panel,
                breaker: BreakerId::new(1),
                state: BreakerState::On,
            }],
            &mut aggregate,
        );
        self.finish_issue(aggregate);
    }

    fn use_fuel_resource(&mut self) {
        let result = self.session.update([GameplayCommand::UseFuelResource {
            player: self.player,
            ship: self.ship,
            item: gameplay::FUEL_RESOURCE_ITEM,
            amount: 1,
        }]);
        self.feedback = format!(
            "ABASTECIMIENTO DE COMBUSTIBLE: {:?}",
            result.first().map(|value| value.status)
        );
        println!("visual gameplay: {}", self.feedback);
    }

    fn restore_life_support(&mut self) {
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into(
            [GameplayCommand::Interact {
                player: self.player,
                target: self.life_support_panel,
            }],
            &mut aggregate,
        );
        self.issue_into(
            [GameplayCommand::SetOxygenAllocation {
                player: self.player,
                panel: self.life_support_panel,
                compartment: gameplay::CompartmentId::new(1),
                allocation: 3,
            }],
            &mut aggregate,
        );
        self.issue_into(
            [GameplayCommand::SetOxygenAllocation {
                player: self.player,
                panel: self.life_support_panel,
                compartment: gameplay::CompartmentId::new(2),
                allocation: 2,
            }],
            &mut aggregate,
        );
        self.finish_issue(aggregate);
    }

    fn accelerate_ship(&mut self, player: EntityId, pilot_station: EntityId, input: GameplayVec3) {
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into(
            [
                GameplayCommand::AccelerateShip {
                    player,
                    pilot_station,
                    input,
                },
                GameplayCommand::AdvanceTime {
                    delta: Duration(100),
                },
            ],
            &mut aggregate,
        );
        self.finish_issue(aggregate);
    }

    fn enter_pilot(&mut self) {
        if self.pilot_mode && !self.mission_one_completed() {
            self.feedback =
                "ERICK: primero debo completar la Misión 1 para preparar la nave.".to_owned();
            return;
        }
        let mut aggregate = FeedbackAggregate::default();
        let accepted = self.issue_into(
            [GameplayCommand::SetShipControlActive {
                player: self.player,
                pilot_station: self.pilot_station,
                active: true,
            }],
            &mut aggregate,
        );
        if accepted {
            let ship_position = self
                .session
                .world()
                .ship_position()
                .unwrap_or(GameplayVec3::ZERO);
            let ship_yaw = self
                .session
                .world()
                .ship_orientation()
                .map(|value| value.yaw_degrees)
                .unwrap_or_default();
            self.pilot_exit_local = self.session.world().player(self.player).map(|value| {
                game::render_bridge::echo7_player_model_position(
                    value.position,
                    ship_position,
                    ship_yaw,
                )
            });
            self.pilot_mode = true;
            self.interior_mode = false;
            self.fire_pressed = false;
            self.boost_pressed = false;
            self.shield_active = false;
            self.feedback = "PILOTAJE DE ECHO-7 ACTIVO - I SALIR, P AUTOPILOTO".to_owned();
        } else {
            self.feedback = aggregate.message();
        }
    }

    fn exit_pilot(&mut self) {
        let mut aggregate = FeedbackAggregate::default();
        let free_autopilot_active = matches!(
            self.session.world().autopilot_state(),
            gameplay::AutopilotState::InTransit { route, next_leg: 1, .. }
                if route.len() == 2 && route[0] == route[1]
        ) && matches!(
            self.session.world().travel_state(),
            gameplay::TravelState::InProgress { .. }
        );
        if free_autopilot_active
            && !self.issue_into(
                [GameplayCommand::CancelAutopilot {
                    player: self.player,
                    pilot_station: self.pilot_station,
                }],
                &mut aggregate,
            )
        {
            self.feedback =
                "ERICK: no puedo abandonar la estación mientras el viaje siga activo.".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        }
        let accepted = self.issue_into(
            [GameplayCommand::SetShipControlActive {
                player: self.player,
                pilot_station: self.pilot_station,
                active: false,
            }],
            &mut aggregate,
        );
        if accepted {
            self.issue([GameplayCommand::SetShipShieldActive {
                player: self.player,
                ship: self.ship,
                active: false,
            }]);
            let released = self.issue_into(
                [GameplayCommand::ReleaseStation {
                    station: self.pilot_station,
                    player: self.player,
                }],
                &mut aggregate,
            );
            if !released {
                self.issue([GameplayCommand::SetShipControlActive {
                    player: self.player,
                    pilot_station: self.pilot_station,
                    active: true,
                }]);
                self.feedback =
                    "ERICK: el viaje fue cancelado, pero no puedo liberar la estación.".to_owned();
                println!("visual gameplay: {}", self.feedback);
                return;
            }
            if let (Some(ship_position), Some(local)) = (
                self.session.world().ship_position(),
                self.pilot_exit_local.take(),
            ) {
                let ship_yaw = self
                    .session
                    .world()
                    .ship_orientation()
                    .map(|value| value.yaw_degrees)
                    .unwrap_or_default();
                let position =
                    game::render_bridge::echo7_model_to_world(local, ship_position, ship_yaw);
                self.session
                    .world_mut()
                    .initialize_player_position(self.player, position);
            }
            self.pilot_mode = false;
            self.interior_mode = true;
            self.flight_keys = FlightKeys::default();
            self.gamepad_flight = GameplayVec3::ZERO;
            self.fire_pressed = false;
            self.boost_pressed = false;
            self.shield_active = false;
            self.feedback = if free_autopilot_active {
                "ERICK: viaje cancelado por abandono de la estación. PILOTAJE FINALIZADO."
                    .to_owned()
            } else {
                "PILOTAJE FINALIZADO - CONTROL DE PERSONAJE RESTAURADO".to_owned()
            };
        } else {
            self.feedback = aggregate.message();
        }
    }

    fn update_pilot_motion(&mut self) {
        if !self.pilot_mode || self.player_defeated {
            return;
        }
        // A/D are the primary steering controls. Arrow keys remain an
        // optional alternative for players who prefer them.
        let yaw = ((self.flight_keys.left as i8 - self.flight_keys.right as i8)
            + self.look_left as i8
            - self.look_right as i8)
            .clamp(-1, 1);
        let pitch = self.look_down as i8 - self.look_up as i8;
        if yaw != 0 || pitch != 0 {
            self.issue([GameplayCommand::RotateShip {
                player: self.player,
                pilot_station: self.pilot_station,
                yaw_input: f32::from(yaw),
                pitch_input: f32::from(pitch),
                delta: Duration(16),
            }]);
        }
        self.issue([GameplayCommand::AccelerateShip {
            player: self.player,
            pilot_station: self.pilot_station,
            input: self.current_flight_input(),
        }]);
        if self.fire_pressed {
            self.issue([GameplayCommand::AttackEnemy {
                player: self.player,
                enemy: self.enemy,
            }]);
        }
    }

    fn rotate_ship_from_input(&mut self, yaw_input: f32, pitch_input: f32) {
        if !self.ship_flight_controls_enabled() {
            return;
        }
        self.issue([GameplayCommand::RotateShip {
            player: self.player,
            pilot_station: self.pilot_station,
            yaw_input,
            pitch_input,
            delta: Duration(16),
        }]);
    }

    fn current_flight_input(&self) -> GameplayVec3 {
        let keyboard = self.flight_keys.input();
        let local = GameplayVec3 {
            // Keyboard A/D steer the ship; lateral thrust is reserved for
            // gamepad input so WASD has one clear meaning.
            x: ((if self.pilot_mode { 0.0 } else { keyboard.x }) + self.gamepad_flight.x)
                .clamp(-1.0, 1.0),
            y: keyboard.y,
            z: (keyboard.z + self.gamepad_flight.z).clamp(-1.0, 1.0),
        };
        let orientation = self.session.world().ship_orientation().unwrap_or_default();
        let yaw = orientation.yaw_degrees.to_radians();
        let pitch = orientation.pitch_degrees.to_radians().clamp(-1.2, 1.2);
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let right = GameplayVec3 {
            x: cos_yaw,
            y: 0.0,
            z: sin_yaw,
        };
        let up = GameplayVec3 {
            x: sin_yaw * sin_pitch,
            y: cos_pitch,
            z: cos_yaw * sin_pitch,
        };
        let forward = GameplayVec3 {
            x: sin_yaw * cos_pitch,
            y: sin_pitch,
            z: -cos_yaw * cos_pitch,
        };
        GameplayVec3 {
            x: right.x * local.x + up.x * local.y - forward.x * local.z,
            y: right.y * local.x + up.y * local.y - forward.y * local.z,
            z: right.z * local.x + up.z * local.y - forward.z * local.z,
        }
    }

    fn mission_one_completed(&self) -> bool {
        self.session
            .mission_progress(crate::el_despertar::EL_DESPERTAR)
            .is_some_and(|mission| {
                mission.state == gameplay::domains::mission::MissionState::Completed
            })
    }

    fn ship_flight_controls_enabled(&self) -> bool {
        // Exterior flight remains available for the legacy/exploration mode.
        // Requiring mission_one_completed here made WASD silently stop
        // applying acceleration while the observation camera still moved.
        self.pilot_mode || self.legacy_exterior_controls
    }

    fn set_gamepad_flight(&mut self, input: GameplayVec3) {
        if self.player_defeated {
            self.gamepad_flight = GameplayVec3::ZERO;
            return;
        }
        if input == self.gamepad_flight {
            return;
        }
        self.gamepad_flight = input;
        if !self.ship_flight_controls_enabled() {
            return;
        }
        self.issue([GameplayCommand::AccelerateShip {
            player: self.player,
            pilot_station: self.pilot_station,
            input: self.current_flight_input(),
        }]);
    }

    fn poll_gamepad(&mut self) {
        self.audio.poll();
        let keyboard_action = self.last_discrete_action.take();
        let frame = self.gamepad.poll();
        self.poll_audio_events();
        self.poll_haptics();
        if self.player_defeated {
            self.gamepad.neutralize();
            self.gamepad_flight = GameplayVec3::ZERO;
            return;
        }
        if !frame.connected {
            self.set_gamepad_flight(GameplayVec3::ZERO);
            return;
        }
        self.set_gamepad_flight(GameplayVec3 {
            x: frame.left_stick.x,
            y: 0.0,
            z: frame.left_stick.y,
        });
        self.observation.adjust_orbit(
            frame.right_stick.x * OBSERVATION_AZIMUTH_STEP,
            -frame.right_stick.y * OBSERVATION_ELEVATION_STEP,
        );
        let mut handled = Vec::new();
        for button in frame.pressed {
            let Some(key) = controller_key(button) else {
                continue;
            };
            if handled.contains(&key) {
                continue;
            }
            if keyboard_action == Some(key) {
                continue;
            }
            handled.push(key);
            self.handle_key(key, ElementState::Pressed);
        }
    }

    fn neutralize_gamepad(&mut self) {
        self.discard_pending_haptics();
        self.gamepad.neutralize();
        if self.player_defeated {
            self.gamepad_flight = GameplayVec3::ZERO;
            return;
        }
        self.set_gamepad_flight(GameplayVec3::ZERO);
    }

    fn poll_haptics(&mut self) {
        let events: Vec<_> = self.session.world().visual_events().copied().collect();
        for event in events {
            if event.id <= self.haptic_event_cursor {
                continue;
            }
            self.haptic_event_cursor = self.haptic_event_cursor.max(event.id);
            let haptic = match event.kind {
                gameplay::GameplayVisualEventKind::Attack if event.source == Some(self.player) => {
                    Some(HapticEvent::AttackExecuted)
                }
                gameplay::GameplayVisualEventKind::EnemyDefeated => {
                    Some(HapticEvent::EnemyDefeated)
                }
                gameplay::GameplayVisualEventKind::ItemCollected
                    if event.source == Some(self.player) =>
                {
                    Some(HapticEvent::ItemCollected)
                }
                _ => None,
            };
            if let Some(haptic) = haptic {
                self.gamepad.request_haptic(haptic);
            }
        }
        let impacted = matches!(
            self.session.world().planet_flight_state(),
            gameplay::PlanetFlightState::Impacted { .. }
        );
        if impacted && !self.last_impacted {
            self.gamepad.request_haptic(HapticEvent::Impact);
        }
        self.last_impacted = impacted;
    }

    fn poll_audio_events(&mut self) {
        let events: Vec<_> = self.session.world().visual_events().copied().collect();
        for event in events {
            let effect = match event.kind {
                gameplay::GameplayVisualEventKind::Attack if event.source == Some(self.player) => {
                    Some(SoundEffect::PlayerAttack)
                }
                gameplay::GameplayVisualEventKind::EnemyDefeated => {
                    Some(SoundEffect::EnemyDefeated)
                }
                gameplay::GameplayVisualEventKind::ItemCollected
                    if event.source == Some(self.player) =>
                {
                    Some(SoundEffect::Collect)
                }
                _ => None,
            };
            if let Some(effect) = effect {
                self.audio.dispatch_event(AudioEvent {
                    id: event.id,
                    effect,
                });
            }
        }

        let flight_state = self.session.world().planet_flight_state();
        let impacted = matches!(flight_state, gameplay::PlanetFlightState::Impacted { .. });
        if impacted && !self.last_audio_impacted {
            self.audio.request_effect(SoundEffect::PlayerImpacted);
        }
        self.last_audio_impacted = impacted;

        let landed = matches!(flight_state, gameplay::PlanetFlightState::Landed { .. });
        if landed && !self.last_landed {
            self.audio.request_effect(SoundEffect::Landing);
        }
        self.last_landed = landed;

        let game_over = self
            .session
            .world()
            .impact_fade_to_black()
            .is_some_and(|fade| fade >= 1.0);
        if game_over && !self.last_game_over {
            self.audio.request_effect(SoundEffect::GameOver);
        }
        self.last_game_over = game_over;
    }

    fn discard_pending_haptics(&mut self) {
        self.haptic_event_cursor = self
            .session
            .world()
            .visual_events()
            .map(|event| event.id)
            .max()
            .unwrap_or(self.haptic_event_cursor);
    }

    fn toggle_observation(&mut self) {
        if self.interior_mode {
            self.feedback =
                "ERICK: sal al exterior o entra en pilotaje para observar los planetas.".to_owned();
            return;
        }
        let now = self.session.world().time().elapsed.0;
        self.observation.toggle(now);
    }

    fn observation_is_active(&self) -> bool {
        self.observation
            .progress(self.session.world().time().elapsed.0)
            >= 0.5
    }

    fn observation_target_label(&self) -> &'static str {
        match self.observation_target {
            Some(id) if id == DESTINATION => "VEYRA",
            Some(id) if id == TESSARA_DESTINATION => "TESSARA",
            Some(id) if id == KHARVEX_DESTINATION => "KHARVEX",
            Some(id) if id == NARETH_DESTINATION => "NARETH",
            _ => "VEYRA",
        }
    }

    fn navigation_destination_label(destination: DestinationId) -> &'static str {
        match destination {
            id if id == DESTINATION => "VEYRA",
            id if id == TESSARA_DESTINATION => "TESSARA",
            id if id == KHARVEX_DESTINATION => "KHARVEX",
            id if id == NARETH_DESTINATION => "NARETH",
            _ => "DESTINO",
        }
    }

    fn free_navigation_proposal_is_stale(&self) -> bool {
        let Some(proposal) = self.session.world().free_navigation_proposal() else {
            return false;
        };
        let world = self.session.world();
        let Some(current_position) = world.ship_position() else {
            return true;
        };
        let Some(destination) = world.destination(proposal.destination) else {
            return true;
        };
        let destination_position = world
            .navigation()
            .planet_position_at(proposal.destination, world.time().elapsed)
            .unwrap_or(destination.position);
        let fuel_available = world
            .navigation()
            .ship_id()
            .and_then(|ship| world.ship_fuel(ship))
            .map(|fuel| fuel.current())
            .unwrap_or_default();
        let destination_moves = world
            .navigation()
            .planet_physics(proposal.destination)
            .is_some_and(|physics| physics.orbit_angular_speed != 0.0);
        proposal.is_stale(
            world.time().elapsed,
            current_position,
            destination_position,
            fuel_available,
            destination_moves,
        )
    }

    fn free_navigation_candidates(&self) -> Vec<DestinationId> {
        self.session
            .world()
            .destinations()
            .filter(|destination| {
                destination.destination_type == gameplay::DestinationType::Planet
                    && destination.discovery != gameplay::DiscoveryState::Unknown
                    && gameplay::is_free_navigation_destination(destination.id)
            })
            .map(|destination| destination.id)
            .collect()
    }

    fn toggle_navigation_selector(&mut self) {
        if self.navigation_selector_open {
            self.navigation_selector_open = false;
            self.free_navigation_candidate = None;
            return;
        }
        let candidates = self.free_navigation_candidates();
        let Some(first) = candidates.first().copied() else {
            self.feedback = "ERICK: no hay planetas disponibles para navegación libre.".to_owned();
            return;
        };
        let confirmed = self.session.world().free_navigation_destination();
        self.free_navigation_candidate = Some(
            confirmed
                .filter(|destination| candidates.contains(destination))
                .unwrap_or(first),
        );
        self.navigation_selector_open = true;
    }

    fn select_free_navigation_candidate(&mut self, direction: i32) {
        let candidates = self.free_navigation_candidates();
        if candidates.is_empty() {
            return;
        }
        let current = self
            .free_navigation_candidate
            .filter(|destination| candidates.contains(destination))
            .unwrap_or(candidates[0]);
        let index = candidates
            .iter()
            .position(|value| *value == current)
            .unwrap_or(0) as i32;
        let next = (index + direction).rem_euclid(candidates.len() as i32) as usize;
        self.free_navigation_candidate = Some(candidates[next]);
    }

    fn confirm_free_navigation_candidate(&mut self) {
        let Some(destination) = self.free_navigation_candidate else {
            return;
        };
        let results = self
            .session
            .update([GameplayCommand::ConfirmFreeNavigationDestination {
                player: self.player,
                pilot_station: self.pilot_station,
                destination,
            }]);
        let status = results.first().map(|result| result.status);
        match status {
            Some(gameplay::CommandStatus::Success { .. }) => {
                self.navigation_selector_open = false;
                self.free_navigation_candidate = None;
                let evaluation = self
                    .session
                    .update([GameplayCommand::EvaluateFreeNavigation {
                        player: self.player,
                        pilot_station: self.pilot_station,
                    }]);
                if matches!(
                    evaluation.first().map(|result| result.status),
                    Some(gameplay::CommandStatus::Success { .. })
                ) {
                    self.feedback = self.free_navigation_proposal_feedback();
                } else {
                    self.feedback = format!(
                        "ERICK: destino de navegación seleccionado: {}.",
                        Self::navigation_destination_label(destination)
                    );
                }
            }
            Some(gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::MissionOneIncomplete,
            }) => {
                self.feedback = "ERICK: primero debemos preparar la nave para el vuelo.".to_owned();
            }
            Some(gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::PilotStationNotOccupiedByPlayer,
            }) => {
                self.feedback = "ERICK: necesito acceso a la estación de pilotaje.".to_owned();
            }
            Some(gameplay::CommandStatus::Rejected { .. })
            | Some(gameplay::CommandStatus::Invalid { .. })
            | None => {
                self.feedback = "ERICK: no puedo aceptar ese destino.".to_owned();
            }
        }
        println!("visual gameplay: {}", self.feedback);
    }

    fn free_navigation_proposal_feedback(&self) -> String {
        let Some(proposal) = self.session.world().free_navigation_proposal() else {
            return "ERICK: no puedo validar la trayectoria con la información disponible."
                .to_owned();
        };
        let evaluation = match proposal.evaluation {
            gameplay::FreeNavigationEvaluation::Viable => "VIABLE",
            gameplay::FreeNavigationEvaluation::NotViable => "NO VIABLE",
            gameplay::FreeNavigationEvaluation::Incomplete => "INCOMPLETA",
        };
        let risks = if proposal.risks.is_empty() {
            "NINGUNO".to_owned()
        } else {
            proposal
                .risks
                .iter()
                .map(|risk| match risk {
                    gameplay::FreeNavigationRisk::SolarCollision => "IMPACTO SOLAR",
                    gameplay::FreeNavigationRisk::PlanetCollision(_) => "COLISIÓN PLANETARIA",
                    gameplay::FreeNavigationRisk::InsufficientFuel { .. } => {
                        "COMBUSTIBLE INSUFICIENTE"
                    }
                    gameplay::FreeNavigationRisk::IncompleteBodyData(_) => {
                        "DATOS GEOMÉTRICOS INCOMPLETOS"
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "ERICK: {} | DESTINO: {} | DISTANCIA: {:.2} | DURACIÓN ESTIMADA: {:.1}s | COMBUSTIBLE: {}/{} | RIESGOS: {}",
            evaluation,
            Self::navigation_destination_label(proposal.destination),
            proposal.distance,
            proposal.duration.0 as f32 / 1000.0,
            proposal.fuel_required,
            proposal.fuel_available,
            risks,
        )
    }

    fn select_observation_planet(&mut self, direction: i32) {
        let planets = game::render_bridge::observable_planets(self.session.world());
        if planets.len() <= 1 {
            return;
        }
        let current = self
            .observation_target
            .filter(|id| planets.contains(id))
            .unwrap_or(DESTINATION);
        let index = planets.iter().position(|id| *id == current).unwrap_or(0) as i32;
        let next = (index + direction).rem_euclid(planets.len() as i32) as usize;
        let selected = planets[next];
        if self.observation_target == Some(selected) {
            return;
        }
        self.observation_target = Some(selected);
        self.observation
            .retarget(self.session.world().time().elapsed.0);
        if selected == NARETH_DESTINATION {
            self.request_nareth_visual_asset();
        }
        self.feedback = format!("OBSERVANDO: {}", self.observation_target_label());
    }

    fn toggle_flight(&mut self, player: EntityId, pilot_station: EntityId) {
        if self.session.world().free_navigation_proposal().is_some() {
            let started = self.issue([GameplayCommand::StartFreeNavigation {
                player,
                pilot_station,
            }]);
            if started {
                self.feedback = "ERICK: autopilotaje libre iniciado.".to_owned();
                println!("visual gameplay: {}", self.feedback);
            }
            return;
        }
        if let Ok(guidance) = self.session.world().guidance_for_player(player) {
            if let Some(
                gameplay::GuidanceAction::TravelTo(destination)
                | gameplay::GuidanceAction::DiscoverDestination(destination),
            ) = guidance.recommended_action
            {
                let mut aggregate = FeedbackAggregate::default();
                if !self.pilot_station_occupied_by(player) {
                    self.issue_into(
                        [GameplayCommand::Interact {
                            player,
                            target: pilot_station,
                        }],
                        &mut aggregate,
                    );
                }
                self.issue_into(
                    [
                        GameplayCommand::SelectDestination {
                            player,
                            pilot_station,
                            destination,
                        },
                        GameplayCommand::StartTravel {
                            player,
                            pilot_station,
                        },
                    ],
                    &mut aggregate,
                );
                self.finish_issue(aggregate);
                return;
            }
        }
        let (origin, destination) = match self.session.world().travel_state() {
            gameplay::TravelState::Completed { destination } if destination == DESTINATION => {
                (DESTINATION, ORIGIN)
            }
            _ => (ORIGIN, DESTINATION),
        };
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into(
            [
                GameplayCommand::Interact {
                    player,
                    target: pilot_station,
                },
                GameplayCommand::PlanAutopilot {
                    player,
                    pilot_station,
                    origin,
                    destination,
                },
                GameplayCommand::StartAutopilot {
                    player,
                    pilot_station,
                },
            ],
            &mut aggregate,
        );
        self.finish_issue(aggregate);
    }

    fn landing_target(&self) -> Option<DestinationId> {
        let world = self.session.world();
        let preferred = self.observation_target.into_iter();
        preferred
            .chain(
                world
                    .destinations()
                    .filter(|destination| destination.destination_type == DestinationType::Planet)
                    .map(|destination| destination.id),
            )
            .find(|destination| {
                world
                    .destination(*destination)
                    .is_some_and(|value| value.destination_type == DestinationType::Planet)
                    && world.landing_available(*destination)
            })
    }

    fn request_landing_from_current_position(&mut self) {
        if self.surface_mode || self.landing_cinematic_started_at.is_some() {
            return;
        }
        let Some(planet) = self.landing_target() else {
            self.feedback = "ACERCATE A UN PLANETA HABITABLE PARA ATERRIZAR".to_owned();
            return;
        };
        if self.session.world_mut().request_planet_landing(planet) {
            self.landing_cinematic_started_at = Some(self.session.world().time().elapsed.0);
            self.pilot_mode = false;
            self.flight_keys = FlightKeys::default();
            self.feedback = format!(
                "ATERRIZAJE INICIADO EN {} — CINEMATICA EN CURSO",
                Self::navigation_destination_label(planet)
            );
        } else {
            self.feedback = "AUN NO ESTAS EN DISTANCIA DE ATERRIZAJE".to_owned();
        }
    }

    fn update_planet_surface_transition(&mut self) {
        let now = self.session.world().time().elapsed.0;
        if let (Some(started), Some(planet)) = (
            self.landing_cinematic_started_at,
            self.session.world().planet_flight_state().landed_planet(),
        ) {
            if now.saturating_sub(started) >= game::render_bridge::LANDING_CINEMATIC_DURATION_MS {
                self.landing_cinematic_started_at = None;
                self.surface_mode = true;
                self.surface_planet = Some(planet);
                self.interior_mode = false;
                self.legacy_exterior_controls = false;
                if let Some(origin) =
                    game::render_bridge::planet_surface_origin(self.session.world(), planet)
                {
                    let origin = GameplayVec3 {
                        x: origin.x,
                        y: origin.y,
                        z: origin.z,
                    };
                    let _ = self
                        .session
                        .world_mut()
                        .initialize_player_position(self.player, origin);
                }
                self.feedback = "ATERRIZAJE COMPLETADO — EXPLORA EL PLANETA. ACERCATE A ECHO-7 Y PULSA I PARA DESPEGAR".to_owned();
            }
        }
        if let Some(started) = self.takeoff_cinematic_started_at {
            if now.saturating_sub(started) >= game::render_bridge::TAKEOFF_CINEMATIC_DURATION_MS {
                if let Some(planet) = self.surface_planet {
                    if let Some(origin) =
                        game::render_bridge::planet_surface_origin(self.session.world(), planet)
                    {
                        let origin = GameplayVec3 {
                            x: origin.x,
                            y: origin.y,
                            z: origin.z + 10.0,
                        };
                        let _ = self.session.world_mut().restore_ship_position(origin);
                    }
                }
                self.session.world_mut().resume_ship_from_landing();
                self.takeoff_cinematic_started_at = None;
                self.surface_mode = false;
                self.surface_planet = None;
                self.pilot_mode = true;
                self.feedback = "DESPEGUE COMPLETADO — ECHO-7 ESTA EN ORBITA".to_owned();
            }
        }
    }

    fn try_takeoff(&mut self) {
        let Some(planet) = self.surface_planet else {
            return;
        };
        let Some(ship_position) =
            game::render_bridge::surface_ship_position(self.session.world(), planet)
        else {
            return;
        };
        let Some(player) = self.session.world().player(self.player) else {
            return;
        };
        let ship_position = GameplayVec3 {
            x: ship_position.x,
            y: ship_position.y,
            z: ship_position.z,
        };
        let distance = ((player.position.x - ship_position.x).powi(2)
            + (player.position.y - ship_position.y).powi(2)
            + (player.position.z - ship_position.z).powi(2))
        .sqrt();
        if distance > game::render_bridge::SURFACE_RETURN_DISTANCE {
            self.feedback = "ACERCATE A ECHO-7 PARA DESPEGAR".to_owned();
            return;
        }
        self.takeoff_cinematic_started_at = Some(self.session.world().time().elapsed.0);
        self.feedback = "DESPEGUE INICIADO — CINEMATICA EN CURSO".to_owned();
    }

    fn pilot_station_occupied_by(&self, player: EntityId) -> bool {
        self.session
            .world()
            .interactable(self.pilot_station)
            .is_some_and(|value| {
                matches!(
                    value.behavior,
                    InteractionBehavior::Station { station }
                        if station.occupant == Some(player)
                )
            })
    }

    fn camera(&self, aspect: f32) -> Result<render_core::Camera, render_core::CameraError> {
        let world = self.session.world();
        if matches!(
            std::env::var("ASTRAL_ECHO_ENFERMERIA_INSPECTION").as_deref(),
            Ok("1") | Ok("true")
        ) {
            let conflicts = [
                "ASTRAL_ECHO_CLONACION_INSPECTION",
                "ASTRAL_ECHO_DESPERTAR_INSPECTION",
                "ASTRAL_ECHO_LOWER_CORRIDOR_INSPECTION",
                "ASTRAL_ECHO_UPPER_CORRIDOR_INSPECTION",
            ]
            .into_iter()
            .filter(
                |name| matches!(std::env::var(name), Ok(value) if value == "1" || value == "true"),
            )
            .collect::<Vec<_>>();
            static REPORTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            REPORTED.get_or_init(move || {
                println!(
                    "ECHO7 inspection mode=enfermeria effective=enfermeria conflicts={conflicts:?} model=ECHO7_Completa_DosCubiertas.obj ready=true"
                );
                println!(
                    "ECHO7 enfermeria camera-local position=(-4.00,18.00,2.538) target=(4.00,13.00,2.538) bounds=x[-28.11,8.11] y[10.89,21.11] z[0,3.35]"
                );
            });
            return game::render_bridge::enfermeria_inspection_camera(world, aspect);
        }
        let despertar = matches!(
            std::env::var("ASTRAL_ECHO_DESPERTAR_INSPECTION").as_deref(),
            Ok("1") | Ok("true")
        );
        let clonacion = matches!(
            std::env::var("ASTRAL_ECHO_CLONACION_INSPECTION").as_deref(),
            Ok("1") | Ok("true")
        );
        if despertar && clonacion {
            println!("ECHO7 inspection conflict=despertar,clonacion effective=despertar");
        }
        if despertar {
            static REPORTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            REPORTED.get_or_init(|| println!("ECHO7 inspection mode=despertar"));
            return game::render_bridge::despertar_inspection_camera(world, aspect);
        }
        if matches!(
            std::env::var("ASTRAL_ECHO_CLONACION_INSPECTION").as_deref(),
            Ok("1") | Ok("true")
        ) {
            static REPORTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            REPORTED.get_or_init(|| println!("ECHO7 inspection mode=clonacion"));
            return game::render_bridge::cloning_inspection_camera(world, aspect);
        }
        if matches!(
            std::env::var("ASTRAL_ECHO_LOWER_CORRIDOR_INSPECTION").as_deref(),
            Ok("1") | Ok("true")
        ) {
            return game::render_bridge::lower_corridor_inspection_camera(world, aspect);
        }
        if matches!(
            std::env::var("ASTRAL_ECHO_UPPER_CORRIDOR_INSPECTION").as_deref(),
            Ok("1") | Ok("true")
        ) {
            return game::render_bridge::upper_corridor_inspection_camera(world, aspect);
        }
        if let Some(started) = self.landing_cinematic_started_at {
            let progress = (world.time().elapsed.0.saturating_sub(started) as f32
                / game::render_bridge::LANDING_CINEMATIC_DURATION_MS as f32)
                .clamp(0.0, 1.0);
            let target = match world.planet_flight_state() {
                gameplay::PlanetFlightState::Landing { planet }
                | gameplay::PlanetFlightState::Landed { planet } => Some(planet),
                _ => self.landing_target(),
            };
            return game::render_bridge::observation_camera_for_destination(
                world, aspect, progress, 0.0, 0.2, target,
            );
        }
        if self.surface_mode {
            return game::render_bridge::interior_camera_for_player(world, self.player, aspect);
        }
        if self.pilot_mode {
            if (self.pilot_camera_distance - game::render_bridge::PILOT_CAMERA_DEFAULT_DISTANCE)
                .abs()
                < f32::EPSILON
            {
                return game::render_bridge::pilot_seated_camera(world, aspect);
            }
            return game::render_bridge::pilot_camera(world, aspect, self.pilot_camera_distance);
        }
        if self.interior_mode {
            return game::render_bridge::interior_camera_for_player(world, self.player, aspect);
        }
        game::render_bridge::observation_camera_for_destination(
            world,
            aspect,
            self.observation.progress(world.time().elapsed.0),
            self.observation.azimuth,
            self.observation.elevation,
            self.observation_target,
        )
    }
}

fn nareth_visual_asset_request() -> VisualAssetRequest {
    VisualAssetRequest {
        resource_id: NARETH_RESOURCE_ID.to_owned(),
        path: Path::new(env!("CARGO_MANIFEST_DIR")).join(NARETH_MODEL_RELATIVE_PATH),
        mesh: MeshHandle::NARETH_PLANET,
        material: VisualAssets::PLANET,
    }
}

pub fn run() -> Result<(), String> {
    let runtime = Rc::new(RefCell::new(VisualBridge::new_visual_runtime()?));
    let (asset_client, asset_host) = visual_asset_channel();
    runtime
        .borrow_mut()
        .attach_visual_asset_client(asset_client);
    let snapshot_runtime = Rc::clone(&runtime);
    let input_runtime = Rc::clone(&runtime);
    let focus_runtime = Rc::clone(&runtime);
    // ASTRAL_MODEL_PARTS_INTEGRATION
    type ModelKey = (render_core::MeshHandle, render_core::MaterialHandle);
    let model_parts: Rc<RefCell<BTreeMap<ModelKey, Vec<render_core::RegisteredModelPart>>>> =
        Rc::new(RefCell::new(BTreeMap::new()));
    let parts_for_snapshot = Rc::clone(&model_parts);
    let parts_for_setup = Rc::clone(&model_parts);
    let inspection_model: Rc<RefCell<Option<render_core::TexturedModel>>> =
        Rc::new(RefCell::new(None));
    let inspection_model_for_snapshot = Rc::clone(&inspection_model);
    let inspection_model_for_setup = Rc::clone(&inspection_model);
    let config = WindowConfig {
        title: "Astral Echo — CPU propio".into(),
        preset: Preset::Low,
        raster_controls: false,
        ..Default::default()
    };
    run_hot_swap_window_with_input_and_scene_and_focus_and_assets(
        config,
        Box::new(move |width, height| {
            let mut bridge = snapshot_runtime.borrow_mut();
            bridge.poll_visual_asset_events();
            bridge.advance_visual_time();
            bridge.update_interior_motion();
            bridge.update_pilot_motion();
            bridge.poll_gamepad();
            let camera = bridge
                .camera(width as f32 / height.max(1) as f32)
                .map_err(|error| format!("cámara visual: {error:?}"))?;
            let mut snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view_and_nareth_and_pilot_and_surface(
                bridge.session.world(),
                Default::default(),
                &camera,
                bridge.interior_mode,
                bridge.nareth_visual_asset_ready(),
                bridge.pilot_mode,
                bridge.surface_planet,
            );
            let echo7_mount_ready = inspection_model_for_snapshot.borrow().is_some();
            if echo7_mount_ready {
                snapshot.nodes.retain(|node| {
                    node.entity
                        != render_core::SnapshotId::Entity(
                            game::render_bridge::INTERIOR_RIGHT_WALL_ENTITY,
                        )
                });
            }
            if bridge.echo7_pick_requested {
                bridge.echo7_pick_requested = false;
                let result = snapshot
                    .nodes
                    .iter()
                    .find(|node| node.mesh == render_core::MeshHandle::SHIP_PLACEHOLDER)
                    .and_then(|node| {
                        let models = inspection_model_for_snapshot.borrow();
                        models
                            .as_ref()
                            .and_then(|model| {
                                let center_pick = matches!(
                                    std::env::var("ASTRAL_ECHO_PICK_CENTER").as_deref(),
                                    Ok("1") | Ok("true")
                                );
                                let sample_y = (height as f32 * 0.12).max(1.0);
                                let sample_x = [
                                    width as f32 * 0.25,
                                    width as f32 * 0.50,
                                    width as f32 * 0.75,
                                ];
                                let mut hits = Vec::new();
                                if center_pick {
                                    let (origin, direction) = echo7_viewport_ray(
                                        camera,
                                        width as f32 * 0.5,
                                        height as f32 * 0.5,
                                        width as f32,
                                        height.max(1) as f32,
                                    );
                                    let hit = echo7_pick_ray(model, origin, direction, node.transform);
                                    println!(
                                        "ECHO7 PICK center pixel=({:.1},{:.1}) ray=({:.4},{:.4},{:.4}) hit={:?}",
                                        width as f32 * 0.5,
                                        height as f32 * 0.5,
                                        direction.x,
                                        direction.y,
                                        direction.z,
                                        hit.as_ref().map(|(object, material, distance)| (object, material, distance))
                                    );
                                    hits.push((width as f32 * 0.5, origin, direction, hit));
                                }
                                for pixel_x in sample_x {
                                    if center_pick {
                                        break;
                                    }
                                    let (origin, direction) = echo7_viewport_ray(
                                        camera,
                                        pixel_x,
                                        sample_y,
                                        width as f32,
                                        height.max(1) as f32,
                                    );
                                    let hit = echo7_pick_ray(model, origin, direction, node.transform);
                                    hits.push((pixel_x, origin, direction, hit));
                                }
                                if !center_pick {
                                    for (pixel_x, _, direction, hit) in &hits {
                                        println!(
                                            "ECHO7 PICK roof-sample pixel=({pixel_x:.1},{sample_y:.1}) ray=({:.4},{:.4},{:.4}) upward={:.4} hit={:?}",
                                            direction.x,
                                            direction.y,
                                            direction.z,
                                            direction.y,
                                            hit.as_ref().map(|(object, material, distance)| (object, material, distance))
                                        );
                                    }
                                }
                                hits.into_iter()
                                    .find_map(|(_, _, _, hit)| hit)
                            })
                    });
                bridge.feedback = match result {
                    Some((object, material, distance)) => format!(
                        "ECHO7 PICK camera=({:.2},{:.2},{:.2}) target=({:.2},{:.2},{:.2}) object={} material={} distance={:.3}",
                        camera.position().x, camera.position().y, camera.position().z,
                        camera.target().x, camera.target().y, camera.target().z,
                        object, material, distance
                    ),
                    None => format!(
                        "ECHO7 PICK sin intersecciÃ³n camera=({:.2},{:.2},{:.2}) target=({:.2},{:.2},{:.2})",
                        camera.position().x, camera.position().y, camera.position().z,
                        camera.target().x, camera.target().y, camera.target().z
                    ),
                };
                println!("{}", bridge.feedback);
            }
            if bridge.hud_visible {
                snapshot.overlay.hud.inventory_open = bridge.inventory_open;
                if bridge.inventory_open {
                    snapshot.overlay.hud.inventory_items = bridge
                        .session
                        .world()
                        .players()
                        .next()
                        .and_then(|player| bridge.session.world().inventory(player.entity_id))
                        .map(|inventory| {
                            inventory
                                .entries()
                                .filter(|(_, amount)| *amount > 0)
                                .map(|(item, amount)| format!("ITEM {}  CANTIDAD {}", item.0, amount))
                                .collect()
                        })
                        .unwrap_or_default();
                }
            } else {
                snapshot.overlay.hud = Default::default();
            }
            if let Some(node) = snapshot.nodes.iter().find(|node| {
                node.entity == render_core::SnapshotId::Destination(NARETH_DESTINATION.value())
            }) {
                bridge.log_nareth_snapshot_mesh(node.mesh);
            }
            // Sustituir cada nodo por sus submallas sin alterar el mundo de juego.
            // El hash de SnapshotId es estable dentro de una ejecucion y distingue
            // instancias del mismo tipo de modelo.
            let parts_by_model = parts_for_snapshot.borrow();
            if !parts_by_model.is_empty() {
                let expanded_capacity = snapshot.nodes.len()
                    + snapshot
                        .nodes
                        .iter()
                        .filter_map(|node| {
                            parts_by_model
                                .get(&(node.mesh, node.material))
                                .map(|parts| parts.len().saturating_sub(1))
                        })
                        .sum::<usize>();
                let mut expanded = Vec::with_capacity(expanded_capacity);
                for node in snapshot.nodes.drain(..) {
                    if let Some(parts) = parts_by_model.get(&(node.mesh, node.material)) {
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        node.entity.hash(&mut hasher);
                        let parent = hasher.finish();
                        render_core::append_model_parts(&mut expanded, &node, parts, parent);
                    } else {
                        expanded.push(node);
                    }
                }
                snapshot.nodes = expanded;
            }
            drop(parts_by_model);
            if snapshot.overlay.game_over {
                snapshot.overlay.hud.interaction_prompt = None;
                snapshot.overlay.hud.contextual_action = None;
                snapshot.overlay.hud.controls_panel = Default::default();
                snapshot.overlay.hud.guidance = None;
                snapshot.overlay.hud.controls = None;
                snapshot.overlay.hud.instructions = None;
                snapshot.overlay.hud.feedback = Some("JUGADOR DERROTADO".to_owned());
            } else {
                snapshot.overlay.hud.interaction_prompt = bridge.contextual_prompt();
                snapshot.overlay.hud.contextual_action = bridge.contextual_action();
                snapshot.overlay.hud.controls_panel = if bridge.started {
                    render_core::HudControlsPanel {
                        open: bridge.controls_panel_open,
                        entries: bridge.controls_panel_entries(),
                    }
                } else {
                    Default::default()
                };
                snapshot.overlay.hud.guidance = bridge.guidance_prompt();
                snapshot.overlay.hud.controls = None;
                snapshot.overlay.hud.instructions = Some(bridge.instructions_hint());
                snapshot.overlay.hud.feedback = Some(bridge.feedback.clone());
            }
            if matches!(
                std::env::var("ASTRAL_ECHO_CLONACION_INSPECTION").as_deref(),
                Ok("1") | Ok("true")
            ) || matches!(
                std::env::var("ASTRAL_ECHO_DESPERTAR_INSPECTION").as_deref(),
                Ok("1") | Ok("true")
            ) || matches!(
                std::env::var("ASTRAL_ECHO_ENFERMERIA_INSPECTION").as_deref(),
                Ok("1") | Ok("true")
            ) {
                snapshot.overlay.hud = Default::default();
            }
            Ok(snapshot)
        }),
        false,
        move |key, state| input_runtime.borrow_mut().handle_key(key, state),
        move |focused| {
            let mut bridge = focus_runtime.borrow_mut();
            bridge.audio.on_focus(focused);
            if !focused {
                bridge.neutralize_gamepad();
            }
        },
        move |scene| {
            let diagnostic_unlit = matches!(
                std::env::var("ASTRAL_ECHO_INTERIOR_DIAGNOSTIC").as_deref(),
                Ok("1") | Ok("true")
            );
            scene.set_lighting_mode(if diagnostic_unlit {
                LightingMode::Unlit
            } else {
                LightingMode::Pbr
            });
            let assets_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models");
            // Registra modelos completos, incluyendo todas sus submallas y texturas.
            // Si falta un archivo, conserva los modelos procedurales existentes.
            for (model_index, definition) in
                crate::model_registry::registered_models().into_iter().enumerate()
            {
                let relative_path = definition.path;
                let mesh = definition.mesh;
                let material = definition.material;
                let path = assets_root.join(relative_path);
                // Reservar rangos diferentes por modelo y separados de handles del motor.
                let first_id = 100_000u32 + (model_index as u32) * 20_000;
                match render_core::register_textured_model(scene, &path, first_id, first_id) {
                    Ok(parts) if !parts.is_empty() => {
                        if relative_path == "echo_7/montajes/ECHO7_Completa_DosCubiertas.obj" {
                            match render_core::load_textured_obj(&path) {
                                Ok(model) => *inspection_model_for_setup.borrow_mut() = Some(model),
                                Err(error) => eprintln!("diagnÃ³stico ECHO-7 no disponible: {error}"),
                            }
                        }
                        if relative_path == "sun/sun.obj" {
                            for part in &parts {
                                scene.set_material(
                                    part.material,
                                    MaterialInfo::new(
                                        render_core::Vec4::ONE,
                                        0.85,
                                        0.0,
                                    )
                                    .with_emissive(
                                        render_core::Vec3::new(0.55, 0.22, 0.045),
                                    ),
                                );
                            }
                            println!(
                                "Sol texturizado: {} submallas con material emisivo",
                                parts.len()
                            );
                        }

                        println!("modelo cargado: {} ({} submallas)", path.display(), parts.len());
                        parts_for_setup.borrow_mut().insert((mesh, material), parts);
                    }
                    Ok(_) => {
                        eprintln!("modelo sin submallas: {}", path.display());
                    }
                    Err(error) => {
                        eprintln!("modelo no cargado {}: {error}; se usa respaldo", path.display());
                    }
                }
            }

            // Modelos de main que aun no tienen entrada en el registro texturizado.
            for (relative_path, mesh, material) in [
                ("moon/moon.obj", render_core::MeshHandle::MOON, VisualAssets::PLANET),
                ("building/building_planet.obj", render_core::MeshHandle::BUILDING_PLANET, VisualAssets::PLANET),
                ("clock/clock_planet.obj", render_core::MeshHandle::TESSARA_PLANET, VisualAssets::PLANET),
                ("mecanic/mecanic_planet.obj", render_core::MeshHandle::KHARVEX_PLANET, VisualAssets::PLANET),
            ] {
                let path = assets_root.join(relative_path);
                match render_core::load_obj(&path) {
                    Ok(mesh_data) => scene.register_mesh(mesh, material, mesh_data),
                    Err(error) => eprintln!("modelo no cargado {}: {error}", path.display()),
                }
            }
            scene.set_material(VisualAssets::SHIP, MaterialInfo::ship());
            scene.set_material(VisualAssets::PLAYER, MaterialInfo::player());
            scene.set_material(VisualAssets::ENEMY, MaterialInfo::enemy_shooter());
            scene.set_material(VisualAssets::ENEMY_CHARGER, MaterialInfo::enemy_charger());
            scene.set_material(VisualAssets::ENEMY_SHIELDED, MaterialInfo::enemy_shielded());
            scene.set_material(VisualAssets::DESTINATION, MaterialInfo::destination());
            scene.set_material(VisualAssets::COLLECTIBLE, MaterialInfo::new(render_core::Vec4::new(1.0, 0.62, 0.04, 1.0), 0.38, 0.25).with_emissive(render_core::Vec3::new(0.16, 0.035, 0.002)));
            scene.set_material(VisualAssets::SUN, MaterialInfo::sun());
            scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
            scene.set_material(VisualAssets::STARS, MaterialInfo::stars());
            scene.set_material(VisualAssets::STATION, MaterialInfo::station());
            scene.set_material(VisualAssets::DERELICT, MaterialInfo::derelict());
            scene.set_material(VisualAssets::ASTEROID, MaterialInfo::asteroid());
        },
        GraphicsDebugConfig {
            directory: std::env::var_os("ASTRAL_ECHO_CAPTURE_DIR").map(std::path::PathBuf::from),
            dual_capture: matches!(
                std::env::var("ASTRAL_ECHO_DUAL_CAPTURE").as_deref(),
                Ok("1") | Ok("true") | Ok("TRUE")
            ),
        },
        asset_host,
    )
    .map(|report| {
        println!(
            "Visual finalizado: transiciones={} transición_gpu_ms={:?}",
            report.transitions, report.last_transition_ms,
        );
    })
    .map_err(|error| format!("ventana visual propia: {error}"))
}

fn controller_key(button: ControllerButton) -> Option<KeyCode> {
    match button {
        ControllerButton::South | ControllerButton::RightTrigger => Some(KeyCode::KeyF),
        ControllerButton::North | ControllerButton::East => Some(KeyCode::KeyO),
        ControllerButton::West => Some(KeyCode::KeyQ),
        ControllerButton::DPadUp => Some(KeyCode::ArrowUp),
        ControllerButton::DPadDown => Some(KeyCode::ArrowDown),
        ControllerButton::DPadLeft => Some(KeyCode::ArrowLeft),
        ControllerButton::DPadRight => Some(KeyCode::ArrowRight),
    }
}

fn guidance_action_label(action: Option<gameplay::GuidanceAction>) -> String {
    match action {
        Some(gameplay::GuidanceAction::UnlockMission(_)) => "M DESBLOQUEAR MISION".to_owned(),
        Some(gameplay::GuidanceAction::StartMission(_)) => "M INICIAR MISION".to_owned(),
        Some(gameplay::GuidanceAction::TravelTo(_)) => "P VIAJAR AL DESTINO".to_owned(),
        Some(gameplay::GuidanceAction::IdentifySignal(_)) => "Q ESCANEAR E IDENTIFICAR".to_owned(),
        Some(gameplay::GuidanceAction::DiscoverDestination(_)) => "P VIAJAR Y DESCUBRIR".to_owned(),
        Some(gameplay::GuidanceAction::DefeatEnemy(_)) => "F ATACAR LA AMENAZA".to_owned(),
        Some(gameplay::GuidanceAction::CollectItem { .. }) => "E RECOGER LA RECOMPENSA".to_owned(),
        Some(gameplay::GuidanceAction::StudyKnowledge(_)) => "Q ESCANEAR Y ESTUDIAR".to_owned(),
        Some(gameplay::GuidanceAction::RepairShip) => "I REPARAR EN LA ESTACION".to_owned(),
        Some(gameplay::GuidanceAction::Interact(_)) => "I INTERACTUAR CON EL OBJETO".to_owned(),
        Some(gameplay::GuidanceAction::CompleteObjective(_)) => {
            "SIGUE LOS REQUISITOS DEL OBJETIVO".to_owned()
        }
        None => "SIN ACCION RECOMENDADA".to_owned(),
    }
}

fn guidance_requirement_label(requirement: gameplay::GuidanceRequirement) -> String {
    match requirement {
        gameplay::GuidanceRequirement::MissionActive(_) => "INICIA LA MISION CON M".to_owned(),
        gameplay::GuidanceRequirement::MissionCompleted(_) => {
            "COMPLETA LA MISION PREVIA".to_owned()
        }
        gameplay::GuidanceRequirement::ObjectiveCompleted(_) => {
            "COMPLETA EL OBJETIVO PREVIO".to_owned()
        }
        gameplay::GuidanceRequirement::KnowledgeAtLeast { .. } => "USA Q PARA ESCANEAR".to_owned(),
        gameplay::GuidanceRequirement::DestinationDiscovered(_) => {
            "DESCUBRE EL DESTINO CON P".to_owned()
        }
        gameplay::GuidanceRequirement::DestinationArrived(_) => "LLEGA AL DESTINO CON P".to_owned(),
        gameplay::GuidanceRequirement::EnemyDefeated(_) => "DERROTA LA AMENAZA CON F".to_owned(),
        gameplay::GuidanceRequirement::ResourceObtained { .. } => {
            "RECOGE EL RECURSO CON E".to_owned()
        }
        gameplay::GuidanceRequirement::WorldActivityCompleted(_) => {
            "COMPLETA LA ACTIVIDAD DEL MUNDO".to_owned()
        }
        gameplay::GuidanceRequirement::DerelictResolved(_) => {
            "RESUELVE LA ESTACION ABANDONADA".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_present::DeferredMeshRegistry;

    #[test]
    fn visual_runtime_requires_explicit_start_and_preserves_initial_campaign_state() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        assert!(!bridge.started);
        assert!(bridge.feedback.contains("ASTRAL ECHO"));
        assert_eq!(bridge.session.world().time().elapsed.0, 0);

        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(bridge.session.world().time().elapsed.0, 0);

        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        assert!(bridge.started);
        assert!(bridge.feedback.contains("ECHO-7"));
        assert_eq!(bridge.session.world().time().elapsed.0, 0);
        assert_eq!(
            bridge
                .session
                .mission_progress(game::campaign::CAMPAIGN_ORDER[0])
                .unwrap()
                .state,
            gameplay::domains::mission::MissionState::Locked
        );
    }

    #[test]
    fn visual_runtime_advances_gameplay_time_while_active() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(bridge.session.world().time().elapsed.0, 100);
        assert_eq!(bridge.session.world().time().tick.0, 1);
    }

    #[test]
    fn visual_runtime_starts_echo7_in_clear_space() {
        let bridge = VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        let world = bridge.session.world();
        let ship = world
            .ship_position()
            .expect("visual runtime should have a ship");
        let distance_squared = |left: GameplayVec3, right: GameplayVec3| {
            let delta = GameplayVec3 {
                x: left.x - right.x,
                y: left.y - right.y,
                z: left.z - right.z,
            };
            delta.x * delta.x + delta.y * delta.y + delta.z * delta.z
        };

        assert_eq!(ship, VISUAL_INITIAL_SHIP_POSITION);
        assert_eq!(
            world.destination(ORIGIN).map(|value| value.position),
            Some(ship)
        );
        assert_eq!(
            world
                .destination(TESSARA_DESTINATION)
                .map(|value| value.position),
            Some(TESSARA_POSITION)
        );
        assert_eq!(
            world
                .destination(KHARVEX_DESTINATION)
                .map(|value| value.position),
            Some(KHARVEX_POSITION)
        );
        assert_eq!(
            world
                .destination(NARETH_DESTINATION)
                .map(|value| value.position),
            Some(NARETH_POSITION)
        );
        assert_eq!(
            world
                .navigation()
                .planet_physics(DESTINATION)
                .expect("Veyra should have planet physics")
                .surface_radius,
            game::render_bridge::PLANET_VISUAL_RADIUS
        );
        let tessara_physics = world
            .navigation()
            .planet_physics(TESSARA_DESTINATION)
            .expect("Tessara should have independent planet physics");
        assert_eq!(
            tessara_physics.orbit_radius,
            game::render_bridge::TESSARA_ORBIT_RADIUS
        );
        assert_eq!(
            tessara_physics.orbit_angular_speed,
            game::render_bridge::TESSARA_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND
        );
        assert_eq!(
            tessara_physics.orbit_center,
            GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0
            }
        );
        assert_eq!(tessara_physics.orbit_start, TESSARA_POSITION);
        assert_eq!(
            tessara_physics.surface_radius,
            game::render_bridge::TESSARA_VISUAL_RADIUS
        );
        let kharvex_physics = world
            .navigation()
            .planet_physics(KHARVEX_DESTINATION)
            .expect("Kharvex should have independent planet physics");
        assert_eq!(
            kharvex_physics.orbit_radius,
            game::render_bridge::KHARVEX_ORBIT_RADIUS
        );
        assert_eq!(
            kharvex_physics.orbit_angular_speed,
            game::render_bridge::KHARVEX_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND
        );
        assert_eq!(
            kharvex_physics.orbit_phase,
            game::render_bridge::KHARVEX_ORBIT_PHASE_RAD
        );
        assert_eq!(
            kharvex_physics.orbit_center,
            GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0
            }
        );
        assert_eq!(
            kharvex_physics.surface_radius,
            game::render_bridge::KHARVEX_VISUAL_RADIUS
        );
        let nareth_physics = world
            .navigation()
            .planet_physics(NARETH_DESTINATION)
            .expect("Nareth should have independent planet physics");
        assert_eq!(
            nareth_physics.orbit_radius,
            game::render_bridge::NARETH_ORBIT_RADIUS
        );
        assert_eq!(
            nareth_physics.orbit_angular_speed,
            game::render_bridge::NARETH_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND
        );
        assert_eq!(nareth_physics.orbit_start, NARETH_POSITION);
        assert_eq!(
            nareth_physics.surface_radius,
            game::render_bridge::NARETH_VISUAL_RADIUS
        );
        assert_eq!(
            world
                .solar_physics()
                .expect("the sun should have collision physics")
                .surface_radius,
            game::render_bridge::SUN_VISUAL_RADIUS
        );
        let nareth_initial = world
            .planet_position_at(NARETH_DESTINATION, Duration(0))
            .expect("Nareth should have an initial orbital position");
        let nareth_delta = GameplayVec3 {
            x: nareth_initial.x - NARETH_POSITION.x,
            y: nareth_initial.y - NARETH_POSITION.y,
            z: nareth_initial.z - NARETH_POSITION.z,
        };
        assert!(
            nareth_delta.x * nareth_delta.x
                + nareth_delta.y * nareth_delta.y
                + nareth_delta.z * nareth_delta.z
                < 1e-8
        );
        assert_ne!(
            world.planet_position_at(NARETH_DESTINATION, Duration(0)),
            world.planet_position_at(NARETH_DESTINATION, Duration(10_000))
        );
        assert!(
            distance_squared(
                ship,
                GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -18.0
                }
            ) > 100.0 * 100.0
        );
        assert!(world
            .destinations()
            .filter(|value| value.id != ORIGIN)
            .all(|value| { distance_squared(ship, value.position) > 30.0 * 30.0 }));
        let enemy = world
            .combatant(bridge.enemy)
            .expect("visual runtime should have its enemy");
        let enemy_distance_squared = distance_squared(ship, enemy.position);
        assert!(enemy_distance_squared > (enemy.weapon.range as f32).powi(2));
        assert!(
            enemy_distance_squared
                <= gameplay::EnemyAiSystem::DETECTION_RADIUS as f32
                    * gameplay::EnemyAiSystem::DETECTION_RADIUS as f32
        );
        assert!(
            distance_squared(
                enemy.position,
                GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -18.0
                }
            ) > 80.0 * 80.0
        );
        assert!(world
            .destinations()
            .all(|value| { distance_squared(enemy.position, value.position) > 10.0 * 10.0 }));
        assert!(world
            .derelicts()
            .all(|value| { distance_squared(enemy.position, value.position) > 10.0 * 10.0 }));
        assert!(world
            .collectibles()
            .all(|value| { distance_squared(enemy.position, value.position) > 10.0 * 10.0 }));
        assert!(world
            .enemies()
            .filter(|value| value.entity != bridge.enemy)
            .all(|value| { distance_squared(ship, value.position) > 30.0 * 30.0 }));
        assert!(world
            .derelicts()
            .all(|derelict| distance_squared(ship, derelict.position) > 30.0 * 30.0));
        assert!(world
            .collectibles()
            .all(|collectible| { distance_squared(ship, collectible.position) > 30.0 * 30.0 }));

        let checkpoint = bridge.session.save_state().serialize();
        let mut restored = ProjectEchoSession::new();
        restored
            .restore_state(
                gameplay::GameplaySaveState::deserialize(&checkpoint)
                    .expect("visual position checkpoint should deserialize"),
            )
            .expect("visual position checkpoint should restore");
        assert_eq!(restored.world().ship_position(), Some(ship));

        let mut restored_visual = VisualBridge::new_visual_runtime()
            .expect("visual runtime should initialize for restoration");
        restored_visual
            .session
            .restore_state(
                gameplay::GameplaySaveState::deserialize(&checkpoint)
                    .expect("visual physics checkpoint should deserialize"),
            )
            .expect("visual physics checkpoint should restore");
        assert!(restored_visual.session.world().solar_physics().is_some());
    }

    #[test]
    fn kharvex_is_observable_but_not_a_free_navigation_candidate() {
        let bridge = VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        assert!(
            game::render_bridge::observable_planets(bridge.session.world())
                .contains(&KHARVEX_DESTINATION)
        );
        assert_eq!(
            bridge.free_navigation_candidates(),
            vec![DESTINATION, TESSARA_DESTINATION]
        );
    }

    #[test]
    fn observation_target_labels_cover_registered_planets_and_unknown_fallback() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");

        bridge.observation_target = Some(DESTINATION);
        assert_eq!(bridge.observation_target_label(), "VEYRA");

        bridge.observation_target = Some(TESSARA_DESTINATION);
        assert_eq!(bridge.observation_target_label(), "TESSARA");

        bridge.observation_target = Some(KHARVEX_DESTINATION);
        assert_eq!(bridge.observation_target_label(), "KHARVEX");

        bridge.observation_target = Some(DestinationId::new(999_999));
        assert_eq!(bridge.observation_target_label(), "VEYRA");
    }

    #[test]
    fn solar_impact_defeats_the_player_and_stops_visual_gameplay() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        assert!(bridge
            .session
            .world_mut()
            .configure_solar_physics(PlanetPhysics {
                destination: gameplay::SUN_DESTINATION_ID,
                orbit_center: GameplayVec3::ZERO,
                orbit_radius: 0.0,
                orbit_start: GameplayVec3::ZERO,
                orbit_axis: GameplayVec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0
                },
                orbit_phase: 0.0,
                orbit_angular_speed: 0.0,
                surface_radius: 12.0,
                ship_radius: 1.0,
                gravity_influence_radius: 80.0,
                gravitational_parameter: 6_000.0,
                atmosphere_height: 0.0,
            }));
        let results = bridge
            .session
            .update([GameplayCommand::SetShipControlActive {
                player: bridge.player,
                pilot_station: bridge.pilot_station,
                active: true,
            }]);
        assert!(matches!(
            results.first().map(|result| &result.status),
            Some(gameplay::CommandStatus::Success { .. })
        ));

        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
        bridge.refresh_player_defeat_state();
        assert!(bridge.session.world().ship_is_destroyed(bridge.ship) == Some(true));
        assert!(bridge
            .session
            .world()
            .combatant(bridge.player)
            .is_some_and(|value| value.is_defeated()));
        assert!(bridge
            .session
            .world()
            .events()
            .iter()
            .any(|event| matches!(event, GameplayEvent::PlanetImpact { planet, .. } if *planet == gameplay::SUN_DESTINATION_ID)));
        assert!(bridge.player_defeated);
    }

    #[test]
    fn introductory_interactables_are_placed_inside_echo7_space() {
        let bridge = VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        let interactables: Vec<_> = bridge.session.world().interactables().collect();

        assert!(interactables.len() >= 6);
        assert!(interactables.iter().all(|value| {
            value.position != GameplayVec3::ZERO
                && value.position.x.abs() < game::render_bridge::ECHO7_INTERIOR_HALF_WIDTH
                && value.position.z.abs() < game::render_bridge::ECHO7_INTERIOR_HALF_DEPTH
        }));
        assert!(interactables.iter().any(|value| {
            matches!(value.behavior, InteractionBehavior::Station { station } if station.station_type == StationType::Piloting)
                && value.position == GameplayVec3 { x: 0.0, y: 0.0, z: 1.5 }
        }));
        assert!(interactables.iter().any(|value| {
            matches!(value.behavior, InteractionBehavior::Station { station } if station.station_type == StationType::Engineering)
                && value.position == GameplayVec3 { x: 2.5, y: 0.0, z: 0.0 }
        }));
    }

    #[test]
    fn visual_runtime_pauses_time_after_player_defeat() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.advance_visual_time_by(Duration(100));
        bridge.player_defeated = true;
        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(bridge.session.world().time().elapsed.0, 100);
        assert_eq!(bridge.session.world().time().tick.0, 1);
    }

    #[test]
    fn controls_panel_is_modal_and_uses_real_mode_assignments() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        let elapsed_before = bridge.session.world().time().elapsed.0;
        bridge.handle_key(KeyCode::F1, ElementState::Pressed);
        assert!(bridge.controls_panel_open);
        assert!(bridge
            .controls_panel_entries()
            .iter()
            .any(|entry| entry == "WASD MOVER - EXPLORACION"));

        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        assert!(!bridge.flight_keys.forward);
        assert_eq!(bridge.session.world().time().elapsed.0, elapsed_before);

        bridge.handle_key(KeyCode::F1, ElementState::Pressed);
        assert!(!bridge.controls_panel_open);
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        assert!(bridge.flight_keys.forward);
    }

    #[test]
    fn observation_target_starts_at_veyra_and_cycles_to_tessara_only_in_pilot_observation() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        assert_eq!(bridge.observation_target, Some(DESTINATION));

        bridge.handle_key(KeyCode::BracketRight, ElementState::Pressed);
        assert_eq!(bridge.observation_target, Some(DESTINATION));

        let before = format!("{:?}", bridge.session.world());
        bridge.pilot_mode = true;
        bridge.observation.from = 1.0;
        bridge.observation.to = 1.0;
        bridge.handle_key(KeyCode::BracketRight, ElementState::Pressed);

        assert_eq!(bridge.observation_target, Some(TESSARA_DESTINATION));
        assert_eq!(before, format!("{:?}", bridge.session.world()));
        assert!(bridge.feedback.contains("TESSARA"));

        bridge.handle_key(KeyCode::BracketLeft, ElementState::Pressed);
        assert_eq!(bridge.observation_target, Some(DESTINATION));
    }

    #[test]
    fn nareth_observation_requests_deferred_asset_and_keeps_fallback() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        let (client, _host) = visual_asset_channel();
        bridge.attach_visual_asset_client(client);
        bridge.pilot_mode = true;
        bridge.observation.from = 1.0;
        bridge.observation.to = 1.0;

        for _ in 0..3 {
            bridge.handle_key(KeyCode::BracketRight, ElementState::Pressed);
        }

        assert_eq!(bridge.observation_target, Some(NARETH_DESTINATION));
        assert_eq!(
            bridge.visual_asset_states.get(NARETH_RESOURCE_ID),
            Some(&VisualAssetState::Loading)
        );
        assert!(bridge.feedback.contains("NARETH"));

        let snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view_and_nareth(
            bridge.session.world(),
            Default::default(),
            &render_core::Camera::demo(),
            false,
            false,
        );
        let nareth = snapshot
            .nodes
            .iter()
            .find(|node| {
                node.entity == render_core::SnapshotId::Destination(NARETH_DESTINATION.value())
            })
            .expect("Nareth should remain in the snapshot while loading");
        assert_eq!(nareth.mesh, render_core::MeshHandle::PLANET_SPHERE);

        bridge
            .visual_asset_states
            .insert(NARETH_RESOURCE_ID.to_owned(), VisualAssetState::Ready);
        let ready_snapshot =
            game::render_bridge::snapshot_from_world_with_camera_and_view_and_nareth(
                bridge.session.world(),
                Default::default(),
                &render_core::Camera::demo(),
                false,
                true,
            );
        let ready_nareth = ready_snapshot
            .nodes
            .iter()
            .find(|node| {
                node.entity == render_core::SnapshotId::Destination(NARETH_DESTINATION.value())
            })
            .expect("Nareth should remain in the snapshot after loading");
        assert_eq!(ready_nareth.mesh, render_core::MeshHandle::NARETH_PLANET);
    }

    #[test]
    fn nareth_visual_request_has_canonical_model_identity() {
        let request = nareth_visual_asset_request();
        assert_eq!(request.resource_id, NARETH_RESOURCE_ID);
        assert_eq!(request.mesh, MeshHandle::NARETH_PLANET);
        assert_eq!(request.material, VisualAssets::PLANET);
        assert!(request
            .path
            .ends_with(Path::new("assets/models/seraphim/Seraphim_planet.obj")));
        assert!(request.path.is_file());
    }

    #[test]
    fn nareth_mesh_requires_gpu_ready_and_ignores_older_generations() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        let (client, mut host) = visual_asset_channel();
        bridge.attach_visual_asset_client(client);
        bridge
            .visual_asset_states
            .insert(NARETH_RESOURCE_ID.to_owned(), VisualAssetState::Ready);
        assert!(!bridge.nareth_visual_asset_ready());

        host.publish_gpu_events([render_core::GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: LodLevel::High,
            renderer_epoch: 3,
            generation: 4,
            state: GpuAssetState::UploadingGpu,
            error: None,
        }]);
        bridge.poll_visual_asset_events();
        assert!(!bridge.nareth_visual_asset_ready());

        host.publish_gpu_events([render_core::GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: LodLevel::High,
            renderer_epoch: 3,
            generation: 4,
            state: GpuAssetState::GpuReady,
            error: None,
        }]);
        bridge.poll_visual_asset_events();
        assert!(bridge.nareth_visual_asset_ready());

        host.publish_gpu_events([render_core::GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: LodLevel::High,
            renderer_epoch: 3,
            generation: 5,
            state: GpuAssetState::GpuFailed,
            error: Some("test GPU failure".into()),
        }]);
        bridge.poll_visual_asset_events();
        assert!(!bridge.nareth_visual_asset_ready());

        host.publish_gpu_events([render_core::GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: LodLevel::High,
            renderer_epoch: 3,
            generation: 4,
            state: GpuAssetState::GpuReady,
            error: None,
        }]);
        bridge.poll_visual_asset_events();
        assert!(!bridge.nareth_visual_asset_ready());

        host.publish_gpu_events([render_core::GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: LodLevel::High,
            renderer_epoch: 4,
            generation: 0,
            state: GpuAssetState::GpuReady,
            error: None,
        }]);
        bridge.poll_visual_asset_events();
        assert!(bridge.nareth_visual_asset_ready());

        host.publish_gpu_events([render_core::GpuAssetEvent {
            mesh: MeshHandle::NARETH_PLANET,
            lod: LodLevel::High,
            renderer_epoch: 3,
            generation: 99,
            state: GpuAssetState::GpuFailed,
            error: Some("old renderer".into()),
        }]);
        bridge.poll_visual_asset_events();
        assert!(bridge.nareth_visual_asset_ready());
    }

    #[test]
    fn observation_target_selection_does_not_change_navigation_or_missions() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.pilot_mode = true;
        bridge.observation.from = 1.0;
        bridge.observation.to = 1.0;
        let navigation_before = format!("{:?}", bridge.session.world().travel_state());
        let missions_before = format!(
            "{:?}",
            bridge.session.world().missions().collect::<Vec<_>>()
        );

        bridge.handle_key(KeyCode::BracketRight, ElementState::Pressed);

        assert_eq!(bridge.observation_target, Some(TESSARA_DESTINATION));
        assert_eq!(
            navigation_before,
            format!("{:?}", bridge.session.world().travel_state())
        );
        assert_eq!(
            missions_before,
            format!(
                "{:?}",
                bridge.session.world().missions().collect::<Vec<_>>()
            )
        );
    }

    #[test]
    fn free_navigation_selector_confirms_planet_without_starting_travel() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        walk_to_real_pilot_control(&mut bridge);
        assert!(bridge.interior_mode);
        for _ in 0..2 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        assert!(bridge.pilot_mode);

        let position_before = bridge.session.world().ship_position();
        let fuel_before = bridge
            .session
            .world()
            .ship_fuel(bridge.ship)
            .expect("ship fuel should exist")
            .current();
        bridge.handle_key(KeyCode::KeyN, ElementState::Pressed);
        assert!(bridge.navigation_selector_open);
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        assert_eq!(bridge.free_navigation_candidate, Some(TESSARA_DESTINATION));
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);

        assert_eq!(
            bridge.session.world().free_navigation_destination(),
            Some(TESSARA_DESTINATION)
        );
        assert!(!bridge.navigation_selector_open);
        assert_eq!(bridge.session.world().ship_position(), position_before);
        assert_eq!(
            bridge
                .session
                .world()
                .ship_fuel(bridge.ship)
                .expect("ship fuel should remain available")
                .current(),
            fuel_before
        );
        assert_eq!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::Idle
        );
        assert_eq!(
            bridge.session.world().autopilot_state(),
            &gameplay::AutopilotState::Inactive
        );
        assert_eq!(bridge.observation_target, Some(DESTINATION));
        assert!(bridge.feedback.contains("TESSARA"));

        bridge.handle_key(KeyCode::KeyN, ElementState::Pressed);
        bridge.handle_key(KeyCode::ArrowLeft, ElementState::Pressed);
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        assert_eq!(
            bridge.session.world().free_navigation_destination(),
            Some(DESTINATION)
        );
    }

    #[test]
    fn free_navigation_selector_does_not_open_outside_pilot_mode() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyN, ElementState::Pressed);
        assert!(!bridge.navigation_selector_open);
        assert_eq!(bridge.session.world().free_navigation_destination(), None);
    }

    #[test]
    fn free_navigation_proposal_starts_and_can_cancel_authoritatively() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        walk_to_real_pilot_control(&mut bridge);
        for _ in 0..2 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        assert!(bridge.pilot_mode);

        bridge.handle_key(KeyCode::KeyN, ElementState::Pressed);
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        let position_before = bridge
            .session
            .world()
            .ship_position()
            .expect("ship position should exist");
        let fuel_before = bridge
            .session
            .world()
            .ship_fuel(bridge.ship)
            .expect("ship fuel should exist")
            .current();

        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        assert!(matches!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::InProgress {
                destination: TESSARA_DESTINATION,
                ..
            }
        ));
        assert!(matches!(
            bridge.session.world().autopilot_state(),
            gameplay::AutopilotState::InTransit {
                destination: TESSARA_DESTINATION,
                ..
            }
        ));
        assert!(
            bridge
                .session
                .world()
                .ship_fuel(bridge.ship)
                .expect("ship fuel should exist")
                .current()
                < fuel_before
        );

        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(500),
        }]);
        assert_ne!(
            bridge.session.world().ship_position(),
            Some(position_before)
        );

        bridge.handle_key(KeyCode::Escape, ElementState::Pressed);
        assert!(matches!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::Cancelled {
                destination: TESSARA_DESTINATION
            }
        ));
        assert!(matches!(
            bridge.session.world().autopilot_state(),
            gameplay::AutopilotState::Interrupted {
                destination: TESSARA_DESTINATION
            }
        ));
        let position_after_cancel = bridge.session.world().ship_position();
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(500),
        }]);
        assert_eq!(
            bridge.session.world().ship_position(),
            position_after_cancel
        );
    }

    #[test]
    fn leaving_pilot_during_free_autopilot_cancels_before_releasing_station() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        walk_to_real_pilot_control(&mut bridge);
        for _ in 0..2 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        assert!(bridge.pilot_mode);

        bridge.handle_key(KeyCode::KeyN, ElementState::Pressed);
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(500),
        }]);
        let position_at_exit = bridge.session.world().ship_position();
        let fuel_at_exit = bridge
            .session
            .world()
            .ship_fuel(bridge.ship)
            .expect("ship fuel should exist")
            .current();

        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);

        assert!(!bridge.pilot_mode);
        assert!(!bridge.pilot_station_occupied_by(bridge.player));
        assert!(matches!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::Cancelled {
                destination: TESSARA_DESTINATION
            }
        ));
        assert!(matches!(
            bridge.session.world().autopilot_state(),
            gameplay::AutopilotState::Interrupted {
                destination: TESSARA_DESTINATION
            }
        ));
        assert_eq!(bridge.session.world().ship_position(), position_at_exit);
        assert_eq!(
            bridge
                .session
                .world()
                .ship_fuel(bridge.ship)
                .expect("ship fuel should remain consumed once")
                .current(),
            fuel_at_exit
        );
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(500),
        }]);
        assert_eq!(bridge.session.world().ship_position(), position_at_exit);
        assert!(bridge.feedback.contains("abandono de la estación"));
    }

    #[test]
    fn free_navigation_confirmation_requires_mission_one_completion() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.pilot_mode = true;
        bridge.handle_key(KeyCode::KeyN, ElementState::Pressed);
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);

        assert_eq!(bridge.session.world().free_navigation_destination(), None);
        assert!(bridge.feedback.contains("preparar la nave"));
        assert_eq!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::Idle
        );
        assert_eq!(
            bridge.session.world().autopilot_state(),
            &gameplay::AutopilotState::Inactive
        );
    }

    #[test]
    fn initial_playable_cycle_keeps_one_session_through_pilot_and_return() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        let session_address = (&bridge.session as *const ProjectEchoSession) as usize;

        bridge.advance_visual_time_by(Duration(100));
        assert!(!bridge.started);
        assert_eq!(bridge.session.world().time().elapsed.0, 0);

        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        assert!(bridge.started);
        assert!(bridge.feedback.contains("ECHO-7"));
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .expect("Mission1 should exist")
                .state,
            gameplay::domains::mission::MissionState::Active
        );

        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        walk_to_real_pilot_control(&mut bridge);
        for _ in 0..2 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .expect("Mission1 should remain available")
                .state,
            gameplay::domains::mission::MissionState::Completed
        );
        assert!(bridge.pilot_mode);
        assert!(!bridge.interior_mode);
        assert_eq!(bridge.session.world().ship_control_active(), Some(true));

        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        assert!(matches!(
            bridge.session.world().autopilot_state(),
            gameplay::AutopilotState::InTransit { .. }
        ));
        assert!(bridge.controls_hint().contains("MODO PILOTAJE"));
        let player_before = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("player should exist")
            .position;
        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(
            bridge
                .session
                .world()
                .player(bridge.player)
                .expect("player should remain present")
                .position,
            player_before
        );

        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        assert!(!bridge.pilot_mode);
        assert!(bridge.interior_mode);
        assert_eq!(bridge.session.world().ship_control_active(), Some(false));
        assert!(bridge.controls_hint().contains("MOVER"));
        assert_eq!(
            session_address,
            (&bridge.session as *const ProjectEchoSession) as usize
        );
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .expect("Mission1 should preserve progress")
                .state,
            gameplay::domains::mission::MissionState::Completed
        );
    }

    #[test]
    fn interior_mode_moves_the_player_without_using_ship_flight() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        let before = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should exist")
            .position;
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.update_interior_motion();
        let after = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should still exist")
            .position;
        assert_ne!(before, after);
        assert_eq!(
            bridge.session.world().ship_position(),
            Some(GameplayVec3::ZERO)
        );
    }

    #[test]
    fn interior_motion_supports_real_stair_ascent_and_descent() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.session.world_mut().initialize_player_position(
            bridge.player,
            GameplayVec3 {
                x: -5.1,
                y: 0.0,
                z: -2.8,
            },
        );
        bridge.flight_keys.backward = true;
        assert!(bridge.current_flight_input().z > 0.0);
        for _ in 0..100 {
            bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
            bridge.update_interior_motion();
        }
        let upper = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("player should remain present")
            .position;
        assert!(upper.y > 2.0, "player did not climb the stair: {upper:?}");

        bridge.flight_keys.backward = false;
        bridge.flight_keys.forward = true;
        for _ in 0..100 {
            bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
            bridge.update_interior_motion();
        }
        let lower = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("player should remain present")
            .position;
        assert!(lower.y < 0.5, "player did not descend the stair: {lower:?}");
    }

    #[test]
    fn interior_controller_reaches_upper_cabin_from_balcony_connection() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        let ship_position = bridge.session.world().ship_position().unwrap();
        let start = game::render_bridge::echo7_model_to_world(
            render_core::Vec3::new(-10.9, 0.0, 4.2),
            ship_position,
            0.0,
        );
        bridge
            .session
            .world_mut()
            .initialize_player_position(bridge.player, start);
        bridge.flight_keys.forward = true;
        let mut reached = false;
        for _ in 0..220 {
            bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
            bridge.update_interior_motion();
            let position = bridge
                .session
                .world()
                .player(bridge.player)
                .unwrap()
                .position;
            let local =
                game::render_bridge::echo7_player_model_position(position, ship_position, 0.0);
            if local.x < -31.0 {
                reached = true;
                assert!((position.y - 4.2 * 0.65).abs() < 0.1);
                break;
            }
        }
        assert!(reached, "the controller did not reach the cabin floor");
    }

    fn drive_to_model(bridge: &mut VisualBridge, waypoint: render_core::Vec3, tolerance: f32) {
        let origin = bridge.session.world().ship_position().unwrap();
        let yaw = bridge
            .session
            .world()
            .ship_orientation()
            .unwrap()
            .yaw_degrees;
        let target = game::render_bridge::echo7_model_to_world(waypoint, origin, yaw);
        for _ in 0..700 {
            let position = bridge
                .session
                .world()
                .player(bridge.player)
                .unwrap()
                .position;
            let dx = target.x - position.x;
            let dz = target.z - position.z;
            let distance = (dx * dx + dz * dz).sqrt();
            if distance < tolerance {
                return;
            }
            bridge.gamepad_flight = GameplayVec3 {
                x: dx / distance,
                y: 0.0,
                z: dz / distance,
            };
            bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
            bridge.update_interior_motion();
        }
        let position = bridge
            .session
            .world()
            .player(bridge.player)
            .unwrap()
            .position;
        let local = game::render_bridge::echo7_player_model_position(position, origin, yaw);
        panic!("controller blocked: player={position:?}, local={local:?}, target={target:?}, model={waypoint:?}");
    }

    fn visit_room(
        bridge: &mut VisualBridge,
        index: usize,
        approach: render_core::Vec3,
        inside: render_core::Vec3,
    ) {
        if index >= 4 && game::render_bridge::ECHO7_ROOM_DOORS[index].center.x.abs() > 1.0 {
            let spec = game::render_bridge::ECHO7_ROOM_DOORS[index];
            drive_to_model(
                bridge,
                render_core::Vec3::new(spec.center.x, 0.0, spec.center.z),
                0.15,
            );
            bridge.gamepad_flight = GameplayVec3::ZERO;
            for _ in 0..20 {
                bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
                bridge.update_interior_motion();
            }
        }
        drive_to_model(bridge, approach, 0.2);
        bridge.gamepad_flight = GameplayVec3::ZERO;
        for _ in 0..20 {
            bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
            bridge.update_interior_motion();
        }
        drive_to_model(bridge, approach, 0.15);
        let door = bridge.room_doors[index];
        if index == 0 {
            assert!(bridge.issue([GameplayCommand::UnlockDoor { door }]));
        }
        assert!(bridge.issue([GameplayCommand::Interact {
            player: bridge.player,
            target: door,
        }]));
        assert!(bridge
            .session
            .world()
            .interactable(door)
            .is_some_and(|value| {
                matches!(
                    value.behavior,
                    gameplay::InteractionBehavior::Door { door }
                        if matches!(door.state, gameplay::domains::door::DoorState::Open)
                )
            }));
        drive_to_model(bridge, inside, 0.2);
        drive_to_model(bridge, approach, 0.2);
    }

    #[test]
    fn lower_deck_room_doors_allow_entry_and_exit_with_real_controller() {
        let mut bridge = VisualBridge::new_visual_runtime().unwrap();
        bridge.started = true;
        assert_eq!(
            bridge.room_doors.len(),
            game::render_bridge::ECHO7_ROOM_DOORS.len()
        );
        for waypoint in [
            render_core::Vec3::new(2.0, 0.84, 0.0),
            render_core::Vec3::new(2.0, -2.5, 0.0),
            render_core::Vec3::new(-2.0, -2.5, 0.0),
            render_core::Vec3::new(-8.0, 0.0, 0.0),
            render_core::Vec3::new(-20.0, 0.0, 0.0),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            1,
            render_core::Vec3::new(-26.0, 0.0, 0.0),
            render_core::Vec3::new(-31.0, 0.0, 0.0),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(-20.0, 0.0, 0.0), 0.2);
        visit_room(
            &mut bridge,
            4,
            render_core::Vec3::new(-20.0, 9.5, 0.0),
            render_core::Vec3::new(-20.0, 14.0, 0.0),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(-20.0, 0.0, 0.0), 0.2);
        visit_room(
            &mut bridge,
            5,
            render_core::Vec3::new(-20.0, -9.5, 0.0),
            render_core::Vec3::new(-20.0, -14.0, 0.0),
        );
        for waypoint in [
            render_core::Vec3::new(-20.0, 0.0, 0.0),
            render_core::Vec3::new(-8.0, 0.0, 0.0),
            render_core::Vec3::new(-2.0, -2.5, 0.0),
            render_core::Vec3::new(2.0, -2.5, 0.0),
            render_core::Vec3::new(2.0, 5.0, 0.0),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            6,
            render_core::Vec3::new(0.0, 9.5, 0.0),
            render_core::Vec3::new(0.0, 14.0, 0.0),
        );
        for waypoint in [
            render_core::Vec3::new(2.0, 5.0, 0.0),
            render_core::Vec3::new(2.0, -2.5, 0.0),
            render_core::Vec3::new(4.0, -8.5, 0.0),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            7,
            render_core::Vec3::new(0.0, -9.5, 0.0),
            render_core::Vec3::new(0.0, -14.0, 0.0),
        );
        for waypoint in [
            render_core::Vec3::new(4.0, -8.5, 0.0),
            render_core::Vec3::new(2.0, -2.5, 0.0),
            render_core::Vec3::new(8.0, 0.0, 0.0),
            render_core::Vec3::new(20.0, 0.0, 0.0),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            8,
            render_core::Vec3::new(20.0, 9.5, 0.0),
            render_core::Vec3::new(20.0, 14.0, 0.0),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(20.0, 0.0, 0.0), 0.2);
        visit_room(
            &mut bridge,
            9,
            render_core::Vec3::new(20.0, -9.5, 0.0),
            render_core::Vec3::new(20.0, -14.0, 0.0),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(20.0, 0.0, 0.0), 0.2);
        visit_room(
            &mut bridge,
            2,
            render_core::Vec3::new(27.5, 0.0, 0.0),
            render_core::Vec3::new(31.0, 0.0, 0.0),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(32.3, 0.0, 0.0), 0.2);
        visit_room(
            &mut bridge,
            3,
            render_core::Vec3::new(32.3, -2.8, 0.0),
            render_core::Vec3::new(32.3, -6.0, 0.0),
        );
    }

    #[test]
    fn upper_deck_room_doors_allow_entry_and_exit_with_real_controller() {
        let mut bridge = VisualBridge::new_visual_runtime().unwrap();
        bridge.started = true;
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        for (index, waypoint) in [
            render_core::Vec3::new(2.0, 0.84, 0.0),
            render_core::Vec3::new(2.0, -2.5, 0.0),
            render_core::Vec3::new(-2.0, -2.5, 0.0),
            render_core::Vec3::new(-4.4, -7.0, 0.0),
            render_core::Vec3::new(-4.1, -7.0, 0.0),
            render_core::Vec3::new(2.9, -7.0, 4.2),
            render_core::Vec3::new(4.8, -7.0, 4.2),
        ]
        .into_iter()
        .enumerate()
        {
            drive_to_model(&mut bridge, waypoint, if index <= 5 { 0.08 } else { 0.2 });
        }
        for waypoint in [
            render_core::Vec3::new(4.0, -8.5, 4.2),
            render_core::Vec3::new(0.0, -9.4, 4.2),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            13,
            render_core::Vec3::new(0.0, -9.5, 4.2),
            render_core::Vec3::new(0.0, -14.0, 4.2),
        );
        for waypoint in [
            render_core::Vec3::new(4.0, -8.5, 4.2),
            render_core::Vec3::new(7.0, -7.0, 4.2),
            render_core::Vec3::new(9.0, -5.0, 4.2),
            render_core::Vec3::new(9.4, 0.0, 4.2),
            render_core::Vec3::new(7.0, 7.0, 4.2),
            render_core::Vec3::new(0.0, 9.4, 4.2),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            12,
            render_core::Vec3::new(0.0, 9.5, 4.2),
            render_core::Vec3::new(0.0, 14.0, 4.2),
        );
        for waypoint in [
            render_core::Vec3::new(0.0, 10.0, 4.2),
            render_core::Vec3::new(4.5, 8.5, 4.2),
            render_core::Vec3::new(7.0, 7.0, 4.2),
            render_core::Vec3::new(9.4, 0.0, 4.2),
            render_core::Vec3::new(11.5, 0.0, 4.2),
            render_core::Vec3::new(20.0, 0.0, 4.2),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            14,
            render_core::Vec3::new(20.0, 9.5, 4.2),
            render_core::Vec3::new(20.0, 14.0, 4.2),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(20.0, 0.0, 4.2), 0.2);
        visit_room(
            &mut bridge,
            15,
            render_core::Vec3::new(20.0, -9.5, 4.2),
            render_core::Vec3::new(20.0, -14.0, 4.2),
        );
        for waypoint in [
            render_core::Vec3::new(20.0, 0.0, 4.2),
            render_core::Vec3::new(11.5, 0.0, 4.2),
            render_core::Vec3::new(9.4, 0.0, 4.2),
            render_core::Vec3::new(7.0, 7.0, 4.2),
            render_core::Vec3::new(0.0, 9.4, 4.2),
            render_core::Vec3::new(-7.0, 7.0, 4.2),
            render_core::Vec3::new(-9.4, 0.0, 4.2),
            render_core::Vec3::new(-11.5, 0.0, 4.2),
            render_core::Vec3::new(-20.0, 0.0, 4.2),
        ] {
            drive_to_model(&mut bridge, waypoint, 0.2);
        }
        visit_room(
            &mut bridge,
            10,
            render_core::Vec3::new(-20.0, 9.5, 4.2),
            render_core::Vec3::new(-20.0, 14.0, 4.2),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(-20.0, 0.0, 4.2), 0.2);
        visit_room(
            &mut bridge,
            11,
            render_core::Vec3::new(-20.0, -9.5, 4.2),
            render_core::Vec3::new(-20.0, -14.0, 4.2),
        );
        drive_to_model(&mut bridge, render_core::Vec3::new(-20.0, 0.0, 4.2), 0.2);
        visit_room(
            &mut bridge,
            0,
            render_core::Vec3::new(-26.0, 0.0, 4.2),
            render_core::Vec3::new(-31.0, 0.0, 4.2),
        );
    }

    fn walk_to_real_pilot_control(bridge: &mut VisualBridge) {
        let origin = bridge.session.world().ship_position().unwrap();
        let waypoints = [
            render_core::Vec3::new(2.0, 0.84, 0.0),
            render_core::Vec3::new(2.0, -2.5, 0.0),
            render_core::Vec3::new(-2.0, -2.5, 0.0),
            render_core::Vec3::new(-4.4, -7.0, 0.0),
            render_core::Vec3::new(-4.1, -7.0, 0.0),
            render_core::Vec3::new(2.9, -7.0, 4.2),
            render_core::Vec3::new(4.8, -7.0, 4.2),
            render_core::Vec3::new(7.0, -7.0, 4.2),
            render_core::Vec3::new(9.0, -5.0, 4.2),
            render_core::Vec3::new(9.4, 0.0, 4.2),
            render_core::Vec3::new(7.0, 7.0, 4.2),
            render_core::Vec3::new(0.0, 9.4, 4.2),
            render_core::Vec3::new(-7.0, 7.0, 4.2),
            render_core::Vec3::new(-9.4, 0.0, 4.2),
            render_core::Vec3::new(-11.5, 0.0, 4.2),
            render_core::Vec3::new(-26.0, 0.0, 4.2),
            game::render_bridge::ECHO7_PILOT_EXIT_LOCAL,
        ];
        for (index, waypoint) in waypoints.into_iter().enumerate() {
            drive_to_model(bridge, waypoint, if index <= 5 { 0.08 } else { 0.3 });
            let position = bridge
                .session
                .world()
                .player(bridge.player)
                .unwrap()
                .position;
            if index == 5 {
                let local = game::render_bridge::echo7_player_model_position(position, origin, 0.0);
                assert!(
                    position.y > 2.0,
                    "stair ascent failed: {position:?}, local={local:?}"
                );
            }
            if index == 15 {
                let closed = bridge
                    .session
                    .world()
                    .interactable(bridge.bridge_door)
                    .is_some_and(|value| {
                        !matches!(
                            value.behavior,
                            gameplay::InteractionBehavior::Door { door }
                                if matches!(door.state, gameplay::domains::door::DoorState::Open)
                        )
                    });
                if closed {
                    assert!(bridge.issue([GameplayCommand::UnlockDoor {
                        door: bridge.bridge_door,
                    }]));
                    assert!(bridge.issue([GameplayCommand::Interact {
                        player: bridge.player,
                        target: bridge.bridge_door,
                    }]));
                }
            }
        }
    }

    #[test]
    fn interior_controller_walks_from_spawn_to_real_pilot_control() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.started = true;
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        walk_to_real_pilot_control(&mut bridge);
        let origin = bridge.session.world().ship_position().unwrap();
        let station = bridge
            .session
            .world()
            .interactable(bridge.pilot_station)
            .unwrap();
        let expected = game::render_bridge::echo7_model_to_world(
            game::render_bridge::ECHO7_PILOT_INTERACTION_LOCAL,
            origin,
            0.0,
        );
        assert_eq!(station.position, expected);
        let entry = bridge
            .session
            .world()
            .player(bridge.player)
            .unwrap()
            .position;
        let walking_snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view(
            bridge.session.world(),
            Default::default(),
            &bridge.camera(16.0 / 9.0).unwrap(),
            true,
        );
        let walking_ship = walking_snapshot
            .nodes
            .iter()
            .find(|node| node.entity == render_core::SnapshotId::Entity(bridge.ship))
            .unwrap();
        assert!(bridge.issue([GameplayCommand::Interact {
            player: bridge.player,
            target: bridge.pilot_station,
        }]));
        bridge.enter_pilot();
        assert!(bridge.pilot_mode);
        let pilot_snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view(
            bridge.session.world(),
            Default::default(),
            &bridge.camera(16.0 / 9.0).unwrap(),
            false,
        );
        let pilot_ship = pilot_snapshot
            .nodes
            .iter()
            .find(|node| node.entity == render_core::SnapshotId::Entity(bridge.ship))
            .unwrap();
        assert_eq!(walking_ship.mesh, pilot_ship.mesh);
        assert_eq!(walking_ship.transform, pilot_ship.transform);
        let seated = bridge.camera(16.0 / 9.0).unwrap();
        let expected_eye = game::render_bridge::echo7_model_to_world(
            game::render_bridge::ECHO7_PILOT_SEATED_LOCAL,
            origin,
            0.0,
        );
        assert!((seated.position().x - expected_eye.x).abs() < 0.01);
        assert!((seated.position().y - expected_eye.y).abs() < 0.01);
        let heading = bridge
            .session
            .world()
            .ship_orientation()
            .unwrap()
            .yaw_degrees;
        bridge.rotate_ship_from_input(1.0, 0.0);
        assert_ne!(
            bridge
                .session
                .world()
                .ship_orientation()
                .unwrap()
                .yaw_degrees,
            heading
        );
        let previous_ship = bridge.session.world().ship_position().unwrap();
        bridge.gamepad_flight = GameplayVec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        for _ in 0..220 {
            bridge.update_pilot_motion();
            bridge.session.update([GameplayCommand::AdvanceTime {
                delta: Duration(50),
            }]);
        }
        let moved_ship = bridge.session.world().ship_position().unwrap();
        assert_ne!(moved_ship, previous_ship);
        let moved_station = bridge
            .session
            .world()
            .interactable(bridge.pilot_station)
            .unwrap()
            .position;
        let moved_yaw = bridge
            .session
            .world()
            .ship_orientation()
            .unwrap()
            .yaw_degrees;
        let expected_station = game::render_bridge::echo7_model_to_world(
            game::render_bridge::ECHO7_PILOT_INTERACTION_LOCAL,
            moved_ship,
            moved_yaw,
        );
        let station_error = ((moved_station.x - expected_station.x).powi(2)
            + (moved_station.y - expected_station.y).powi(2)
            + (moved_station.z - expected_station.z).powi(2))
        .sqrt();
        assert!(station_error < 0.01);
        let old_point_distance = ((moved_station.x - expected.x).powi(2)
            + (moved_station.y - expected.y).powi(2)
            + (moved_station.z - expected.z).powi(2))
        .sqrt();
        assert!(old_point_distance > 3.0);
        let probe = bridge.session.world_mut().create_player().unwrap();
        bridge
            .session
            .world_mut()
            .initialize_player_position(probe, expected);
        let old_point_attempt = bridge.session.update([GameplayCommand::Interact {
            player: probe,
            target: bridge.pilot_station,
        }]);
        assert!(matches!(
            old_point_attempt.first().map(|result| &result.status),
            Some(gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::OutOfInteractionRange
            })
        ));
        bridge.exit_pilot();
        assert!(bridge.interior_mode);
        let exit = bridge
            .session
            .world()
            .player(bridge.player)
            .unwrap()
            .position;
        let yaw = bridge
            .session
            .world()
            .ship_orientation()
            .unwrap()
            .yaw_degrees;
        let exit_local = game::render_bridge::echo7_player_model_position(exit, moved_ship, yaw);
        let entry_local = game::render_bridge::echo7_player_model_position(entry, origin, 0.0);
        assert!((exit_local - entry_local).length() < 0.01);
        assert!(game::render_bridge::echo7_upper_or_stair_surface_contains(
            exit, moved_ship, yaw
        ));
        let beyond_cabin = game::render_bridge::echo7_model_to_world(
            render_core::Vec3::new(-43.5, 0.0, 4.2),
            moved_ship,
            yaw,
        );
        assert!(!game::render_bridge::echo7_upper_or_stair_surface_contains(
            beyond_cabin,
            moved_ship,
            yaw,
        ));
        let walking_camera = bridge.camera(16.0 / 9.0).unwrap();
        assert!(
            (walking_camera.position().y - exit.y - game::render_bridge::ECHO7_INTERIOR_EYE_HEIGHT)
                .abs()
                < 0.01
        );
        bridge.gamepad_flight = GameplayVec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
        bridge.last_interior_update = Instant::now() - std::time::Duration::from_millis(50);
        bridge.update_interior_motion();
        assert_ne!(
            bridge
                .session
                .world()
                .player(bridge.player)
                .unwrap()
                .position,
            exit
        );
        assert!(bridge.issue([GameplayCommand::Interact {
            player: bridge.player,
            target: bridge.pilot_station,
        }]));
        bridge.enter_pilot();
        assert!(bridge.pilot_mode);
    }

    #[test]
    fn pilot_mode_is_entered_from_the_pilot_console_and_exits_to_character_control() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.guided_interaction();
        assert!(
            bridge.pilot_mode,
            "the nearby piloting console should enter pilot mode"
        );
        assert!(!bridge.interior_mode);
        assert_eq!(bridge.session.world().ship_control_active(), Some(true));

        let relocated_ship = GameplayVec3 {
            x: 96.0,
            y: 4.0,
            z: -72.0,
        };
        bridge
            .session
            .world_mut()
            .restore_ship_position(relocated_ship)
            .expect("the piloted ship should relocate for this regression scenario");
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        assert!(!bridge.pilot_mode);
        assert!(bridge.interior_mode);
        assert_eq!(bridge.session.world().ship_control_active(), Some(false));
        let ship_after_exit = bridge.session.world().ship_position().unwrap();
        assert_eq!(ship_after_exit, relocated_ship);
        assert_eq!(
            bridge.session.world().player(bridge.player).unwrap().position,
            relocated_ship
        );
        assert_eq!(
            bridge
                .session
                .world()
                .interactable(bridge.pilot_station)
                .unwrap()
                .position,
            GameplayVec3 {
                x: relocated_ship.x,
                y: relocated_ship.y,
                z: relocated_ship.z + 1.5,
            }
        );

        let status_position = bridge
            .session
            .world()
            .interactable(bridge.status_console)
            .unwrap()
            .position;
        bridge
            .session
            .world_mut()
            .initialize_player_position(bridge.player, status_position);
        bridge.guided_interaction();
        assert!(!bridge.pilot_mode, "I should use the nearby status console");
        assert!(matches!(
            bridge
                .session
                .world()
                .interactable(bridge.status_console)
                .unwrap()
                .behavior,
            InteractionBehavior::Console {
                state: gameplay::ConsoleState::On,
                ..
            }
        ));

        let heading_after_exit = bridge.session.world().ship_orientation().unwrap();
        let pilot_position = bridge
            .session
            .world()
            .interactable(bridge.pilot_station)
            .unwrap()
            .position;
        bridge
            .session
            .world_mut()
            .initialize_player_position(bridge.player, pilot_position);
        bridge.guided_interaction();
        assert!(bridge.pilot_mode);
        assert_eq!(
            bridge.session.world().ship_position(),
            Some(ship_after_exit)
        );
        assert_eq!(
            bridge.session.world().ship_orientation(),
            Some(heading_after_exit)
        );
    }

    #[test]
    fn pilot_mode_moves_and_rotates_the_authoritative_ship_only() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.guided_interaction();
        assert!(bridge.pilot_mode);
        let player_before = bridge
            .session
            .world()
            .player(bridge.player)
            .unwrap()
            .position;
        let ship_before = bridge.session.world().ship_position().unwrap();
        let orientation_before = bridge.session.world().ship_orientation().unwrap();

        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        bridge.update_pilot_motion();

        assert_ne!(bridge.session.world().ship_position().unwrap(), ship_before);
        assert_ne!(
            bridge.session.world().ship_orientation().unwrap(),
            orientation_before
        );
        assert_eq!(
            bridge
                .session
                .world()
                .player(bridge.player)
                .unwrap()
                .position,
            player_before
        );
    }

    #[test]
    fn pilot_camera_remains_behind_the_propulsors_after_ship_rotation() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.guided_interaction();
        bridge.issue([GameplayCommand::RotateShip {
            player: bridge.player,
            pilot_station: bridge.pilot_station,
            yaw_input: 1.0,
            pitch_input: 0.0,
            delta: Duration(1_200),
        }]);

        let ship = game::render_bridge::visual_ship_position(bridge.session.world())
            .expect("ship should be visible");
        let camera = bridge.camera(16.0 / 9.0).expect("pilot camera should be valid");
        assert!(camera.position().x > ship.x + 12.0);
        assert!(camera.target().x > ship.x + 2.5);
        assert!(camera.position().x > camera.target().x);
        assert!(camera.view_projection_matrix().is_finite());
    }

    #[test]
    fn pilot_shield_mitigates_damage_and_resets_when_disabled() {
        let mut bridge =
            VisualBridge::new_with_mode(false).expect("visual bridge should initialize");
        assert_eq!(bridge.session.world().ship_control_active(), Some(true));
        let shield = bridge
            .session
            .update([GameplayCommand::SetShipShieldActive {
                player: bridge.player,
                ship: bridge.ship,
                active: true,
            }]);
        assert!(matches!(
            shield.first().map(|result| &result.status),
            Some(gameplay::CommandStatus::Success { changed: true })
        ));
        assert!(bridge.session.world().ship_shield_active(bridge.ship));
        bridge.session.update([GameplayCommand::DamageShip {
            ship: bridge.ship,
            amount: 10,
        }]);
        assert_eq!(
            bridge
                .session
                .world()
                .ship_integrity(bridge.ship)
                .unwrap()
                .current(),
            95
        );
        bridge
            .session
            .update([GameplayCommand::SetShipShieldActive {
                player: bridge.player,
                ship: bridge.ship,
                active: false,
            }]);
        bridge.session.update([GameplayCommand::DamageShip {
            ship: bridge.ship,
            amount: 10,
        }]);
        assert_eq!(
            bridge
                .session
                .world()
                .ship_integrity(bridge.ship)
                .unwrap()
                .current(),
            85
        );
    }

    #[test]
    fn interior_rotation_continues_until_key_release() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        let first = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should exist")
            .orientation
            .yaw_degrees;
        bridge.update_interior_motion();
        let continued = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should exist")
            .orientation
            .yaw_degrees;
        assert!(continued > first);

        bridge.handle_key(KeyCode::ArrowRight, ElementState::Released);
        bridge.update_interior_motion();
        let stopped = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should exist")
            .orientation
            .yaw_degrees;
        assert_eq!(stopped, continued);
    }

    #[test]
    fn interior_prompt_identifies_the_nearest_real_interactable() {
        let bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        assert!(bridge
            .contextual_prompt()
            .is_some_and(|prompt| prompt.contains("[I]") && prompt.contains("PILOTAJE")));
    }

    #[test]
    fn contextual_prompt_disappears_when_player_leaves_interaction_range() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        for _ in 0..2_000 {
            bridge.update_interior_motion();
        }
        assert!(bridge.contextual_prompt().is_none());
    }

    #[test]
    fn guidance_prompt_uses_guidance_and_real_controls() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        assert!(bridge
            .guidance_prompt()
            .is_some_and(|value| value.contains("M")));
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert!(bridge
            .guidance_prompt()
            .is_some_and(|value| value.contains("I") || value.contains("Q")));
        assert!(bridge.controls_hint().contains("I USAR"));
    }

    #[test]
    fn instructions_follow_title_exploration_and_pilot_modes() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.started = false;
        assert_eq!(bridge.instructions_hint(), "PULSA ENTER PARA COMENZAR");
        assert_eq!(bridge.controls_hint(), "ENTER COMENZAR");

        bridge.start_game();
        assert!(bridge
            .instructions_hint()
            .contains("PULSA LA TECLA INDICADA"));
        bridge.guided_interaction();
        assert!(bridge.pilot_mode);
        assert!(bridge.instructions_hint().contains("PILOTAJE ACTIVO"));
        assert!(bridge.contextual_prompt().is_none());
    }

    #[test]
    fn interior_motion_stays_inside_the_player_clearance_boundary() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.handle_key(KeyCode::KeyD, ElementState::Pressed);
        for _ in 0..240 {
            bridge.update_interior_motion();
        }
        let position = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should exist")
            .position;
        assert!(
            position.x < 8.4,
            "player crossed the interior clearance: {position:?}"
        );
    }

    #[test]
    fn interior_diagonal_motion_stays_inside_both_clearance_boundaries() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyD, ElementState::Pressed);
        for _ in 0..480 {
            bridge.update_interior_motion();
        }
        let position = bridge
            .session
            .world()
            .player(bridge.player)
            .expect("visual player should still exist")
            .position;
        assert!(
            position.x.abs() <= 7.8,
            "player escaped ECHO-7 width: {position:?}"
        );
        assert!(
            position.z.abs() <= 5.8,
            "player escaped ECHO-7 depth: {position:?}"
        );
    }

    #[test]
    fn character_camera_follows_the_validated_player_position() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        let before = bridge
            .camera(16.0 / 9.0)
            .expect("character camera should be valid")
            .position();
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.update_interior_motion();
        let after = bridge
            .camera(16.0 / 9.0)
            .expect("character camera should remain valid")
            .position();
        assert_ne!(before, after);
        assert!(after.y > -1.2 && after.y < 6.4);
    }

    #[test]
    fn echo7_interior_uses_one_full_ship_mesh_with_visible_hull() {
        let bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        let camera = bridge
            .camera(16.0 / 9.0)
            .expect("character camera should be valid");
        let snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view(
            bridge.session.world(),
            Default::default(),
            &camera,
            true,
        );
        let echo7 = snapshot
            .nodes
            .iter()
            .find(|node| node.entity == render_core::SnapshotId::Entity(bridge.ship));
        assert_eq!(
            echo7.map(|node| node.mesh),
            Some(game::render_bridge::ECHO7_MESH)
        );
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .filter(|node| node.mesh == game::render_bridge::ECHO7_MESH)
                .count(),
            1
        );
        assert_ne!(
            game::render_bridge::ECHO7_MESH,
            game::render_bridge::STATION_MESH
        );
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .filter(|node| node.mesh == game::render_bridge::STATION_MESH)
                .count(),
            0
        );
        assert!(
            snapshot
                .nodes
                .iter()
                .filter(|node| node.mesh == game::render_bridge::PLACEHOLDER_MESH)
                .count()
                >= 2
        );
    }

    #[test]
    fn echo7_exterior_uses_full_ship_representation() {
        let bridge = VisualBridge::new_with_mode(false).expect("visual bootstrap should work");
        let camera = bridge
            .camera(16.0 / 9.0)
            .expect("character camera should be valid");
        let snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view(
            bridge.session.world(),
            Default::default(),
            &camera,
            false,
        );
        let echo7 = snapshot
            .nodes
            .iter()
            .find(|node| node.entity == render_core::SnapshotId::Entity(bridge.ship));
        assert_eq!(
            echo7.map(|node| node.mesh),
            Some(game::render_bridge::ECHO7_MESH)
        );
        assert_ne!(
            game::render_bridge::ECHO7_MESH,
            game::render_bridge::STATION_MESH
        );
    }

    #[test]
    fn controller_bindings_reuse_existing_keyboard_actions() {
        assert_eq!(controller_key(ControllerButton::South), Some(KeyCode::KeyF));
        assert_eq!(
            controller_key(ControllerButton::RightTrigger),
            Some(KeyCode::KeyF)
        );
        assert_eq!(controller_key(ControllerButton::West), Some(KeyCode::KeyQ));
        assert_eq!(controller_key(ControllerButton::North), Some(KeyCode::KeyO));
        assert_eq!(
            controller_key(ControllerButton::DPadLeft),
            Some(KeyCode::ArrowLeft)
        );
    }

    #[test]
    fn feedback_aggregate_classifies_all_result_combinations() {
        let success = gameplay::CommandResult::success(true);
        let failure =
            gameplay::CommandResult::rejected(gameplay::CommandRejection::InvalidDuration);

        let cases = [
            (
                &[success, success][..],
                FeedbackOutcome::Complete,
                true,
                "OPERACION COMPLETADA",
            ),
            (
                &[failure, failure][..],
                FeedbackOutcome::Failed,
                false,
                "OPERACION FALLIDA",
            ),
            (
                &[success, failure][..],
                FeedbackOutcome::Partial,
                false,
                "PARCIALMENTE COMPLETADA",
            ),
            (
                &[][..],
                FeedbackOutcome::Empty,
                false,
                "SIN RESULTADO EVALUABLE",
            ),
        ];
        for (results, outcome, complete, label) in cases {
            let mut aggregate = FeedbackAggregate::default();
            let commands = vec![
                GameplayCommand::AdvanceTime {
                    delta: Duration(100),
                };
                results.len()
            ];
            aggregate.record(&commands, results, ["evento".to_owned()]);
            assert_eq!(aggregate.outcome(), outcome);
            assert_eq!(aggregate.is_complete(), complete);
            assert!(aggregate.message().contains(label));
            if !results.is_empty() {
                assert!(aggregate.message().contains("AdvanceTime"));
            }
            assert!(aggregate.message().contains("eventos=evento"));
        }
    }

    #[test]
    fn issue_aggregates_results_and_starts_each_operation_clean() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let first = bridge.issue([
            GameplayCommand::AdvanceTime {
                delta: Duration(100),
            },
            GameplayCommand::AdvanceTime { delta: Duration(0) },
        ]);
        assert!(!first);
        assert!(bridge.feedback.contains("PARCIALMENTE COMPLETADA"));
        assert!(bridge.feedback.contains("comandos=2 exitosos=1 fallidos=1"));

        let empty = bridge.issue(std::iter::empty::<GameplayCommand>());
        assert!(!empty);
        assert!(bridge.feedback.contains("SIN RESULTADO EVALUABLE"));

        let next = bridge.issue([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
        assert!(next);
        assert!(bridge.feedback.contains("OPERACION COMPLETADA"));
        assert!(bridge.feedback.contains("comandos=1 exitosos=1 fallidos=0"));
    }

    #[test]
    fn player_defeat_enters_terminal_visual_state_and_blocks_world_commands() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");

        for _ in 0..20 {
            bridge.issue([GameplayCommand::AdvanceTime {
                delta: Duration(1_000),
            }]);
            if bridge.player_defeated {
                break;
            }
        }

        assert!(bridge.player_defeated);
        assert!(bridge
            .session
            .world()
            .events()
            .iter()
            .any(|event| matches!(event, GameplayEvent::PlayerDefeated { player } if *player == bridge.player)));
        assert!(bridge.feedback.contains("JUGADOR DERROTADO"));

        let defeated_time = bridge.session.world().time();
        let defeated_feedback = bridge.feedback.clone();
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        bridge.set_gamepad_flight(GameplayVec3 {
            x: 1.0,
            y: 0.0,
            z: -1.0,
        });
        bridge.poll_gamepad();

        assert_eq!(bridge.session.world().time(), defeated_time);
        assert_eq!(bridge.gamepad_flight, GameplayVec3::ZERO);
        assert_eq!(bridge.feedback, defeated_feedback);
        let snapshot =
            game::render_bridge::snapshot_from_world(bridge.session.world(), Default::default());
        assert!(snapshot.overlay.game_over);
        assert_eq!(
            snapshot.overlay.hud.feedback.as_deref(),
            Some("JUGADOR DERROTADO")
        );
        assert!(snapshot.nodes.iter().all(|node| {
            node.transform.translation.is_finite()
                && node.transform.rotation.is_finite()
                && node.transform.scale.is_finite()
        }));
    }

    #[test]
    fn interior_start_keeps_enemy_outside_immediate_weapon_range() {
        let mut bridge =
            VisualBridge::new_with_mode_and_enemy_distance(true, true, GameplayVec3::ZERO, true)
                .expect("visual bridge should initialize");
        let enemy = bridge
            .session
            .world()
            .combatant(bridge.enemy)
            .copied()
            .expect("visual enemy should exist");
        assert!(!enemy.active, "the introductory threat must start inactive");
        let player = bridge
            .session
            .world()
            .combatant(bridge.player)
            .copied()
            .expect("visual player should exist");
        let dx = f64::from(enemy.position.x - player.position.x);
        let dy = f64::from(enemy.position.y - player.position.y);
        let dz = f64::from(enemy.position.z - player.position.z);
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        assert!(distance > f64::from(enemy.weapon.range));

        for _ in 0..4 {
            bridge.advance_visual_time_by(Duration(500));
        }

        assert!(!bridge.player_defeated);
        assert_eq!(
            bridge
                .session
                .world()
                .combatant(bridge.enemy)
                .unwrap()
                .position,
            enemy.position
        );
        assert!(!bridge.session.world().events().iter().any(|event| {
            matches!(event, GameplayEvent::CombatDamageApplied { target, .. } if *target == bridge.player)
        }));
    }

    #[test]
    fn introductory_threat_activates_once_after_mission_one_completion() {
        let mut bridge =
            VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert!(
            !bridge
                .session
                .world()
                .combatant(bridge.enemy)
                .unwrap()
                .active
        );

        for _ in 0..3 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        walk_to_real_pilot_control(&mut bridge);
        for _ in 0..2 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }

        assert_eq!(
            bridge
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .unwrap()
                .state,
            gameplay::domains::mission::MissionState::Completed
        );
        assert!(
            bridge
                .session
                .world()
                .combatant(bridge.enemy)
                .unwrap()
                .active
        );
        assert_eq!(
            bridge
                .session
                .world()
                .combatants()
                .filter(|combatant| {
                    !combatant.is_player
                        && combatant.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            1
        );
        assert!(!bridge.introductory_threat_pending);

        let position_before = bridge
            .session
            .world()
            .combatant(bridge.enemy)
            .unwrap()
            .position;
        bridge.advance_visual_time_by(Duration(1_000));
        assert_ne!(
            bridge
                .session
                .world()
                .combatant(bridge.enemy)
                .unwrap()
                .position,
            position_before
        );
        for _ in 0..5 {
            bridge.advance_visual_time_by(Duration(1_000));
        }
        assert!(!bridge.session.world().events().iter().any(|event| {
            matches!(event, GameplayEvent::CombatDamageApplied { target, .. } if *target == bridge.player)
        }));
        assert_eq!(
            bridge
                .session
                .world()
                .combatants()
                .filter(|combatant| {
                    !combatant.is_player
                        && combatant.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            1
        );
    }

    #[test]
    fn composite_controls_report_all_commands_without_rollback() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");

        bridge.restore_energy();
        assert!(bridge.feedback.contains("comandos=2"));
        bridge.restore_life_support();
        assert!(bridge.feedback.contains("comandos=3"));

        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        assert!(bridge.feedback.contains("comandos=3"));
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        assert!(bridge.feedback.contains("comandos=2"));
    }

    #[test]
    fn visual_retry_uses_authoritative_command_and_can_recover_after_failure() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let player = bridge.player;
        bridge.session.update([GameplayCommand::UnlockMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        bridge.session.update([GameplayCommand::SetPlayerActive {
            player,
            active: false,
        }]);
        bridge.session.update([GameplayCommand::StartMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);

        bridge.handle_key(KeyCode::KeyT, ElementState::Pressed);
        assert!(bridge.feedback.contains("NO DISPONIBLE"));
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .unwrap()
                .required_completed,
            0
        );

        bridge.session.update([GameplayCommand::SetPlayerActive {
            player,
            active: true,
        }]);
        bridge.handle_key(KeyCode::KeyT, ElementState::Pressed);
        assert!(bridge.feedback.contains("PREPARADA"));
        assert_eq!(
            bridge
                .session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            1
        );
    }

    #[test]
    fn visual_retry_is_discrete_and_released_key_does_not_request_recovery() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let before = bridge.feedback.clone();
        bridge.handle_key(KeyCode::KeyT, ElementState::Released);
        assert_eq!(bridge.feedback, before);
    }

    #[test]
    fn flight_keys_produce_distinct_individual_directions() {
        let cases = [
            (
                KeyCode::KeyW,
                GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -1.0,
                },
            ),
            (
                KeyCode::KeyS,
                GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
            ),
            (
                KeyCode::KeyA,
                GameplayVec3 {
                    x: -1.0,
                    y: 0.0,
                    z: 0.0,
                },
            ),
            (
                KeyCode::KeyD,
                GameplayVec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
            ),
        ];

        for (key, expected) in cases {
            let mut keys = FlightKeys::default();
            assert!(keys.set(key, true));
            assert_eq!(keys.input(), expected);
        }
    }

    #[test]
    fn flight_keys_support_opposite_and_combined_inputs() {
        let mut keys = FlightKeys::default();
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyD, true);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 1.0,
                y: 0.0,
                z: -1.0
            }
        );

        keys.set(KeyCode::KeyW, false);
        keys.set(KeyCode::KeyA, true);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );

        keys.set(KeyCode::KeyD, false);
        keys.set(KeyCode::KeyS, true);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: -1.0,
                y: 0.0,
                z: 1.0
            }
        );
    }

    #[test]
    fn flight_keys_support_vertical_space_navigation() {
        let mut keys = FlightKeys::default();
        keys.set(KeyCode::Space, true);
        assert_eq!(keys.input().y, 1.0);
        keys.set(KeyCode::Space, false);
        keys.set(KeyCode::ControlLeft, true);
        assert_eq!(keys.input().y, -1.0);
        keys.set(KeyCode::ControlLeft, false);
        assert_eq!(keys.input().y, 0.0);
    }

    #[test]
    fn releasing_a_flight_key_preserves_other_active_inputs() {
        let mut keys = FlightKeys::default();
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyA, true);
        keys.set(KeyCode::KeyW, false);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0
            }
        );

        keys.set(KeyCode::KeyA, false);
        assert_eq!(keys.input(), GameplayVec3::ZERO);
    }

    #[test]
    fn simultaneous_flight_key_sequences_keep_only_remaining_contributions() {
        let mut keys = FlightKeys::default();
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyD, false);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0
            }
        );

        keys = FlightKeys::default();
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyS, true);
        keys.set(KeyCode::KeyD, false);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0
            }
        );

        keys = FlightKeys::default();
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyD, false);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0
            }
        );

        keys = FlightKeys::default();
        keys.set(KeyCode::KeyS, true);
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyS, false);
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn repeated_press_does_not_accumulate_and_release_clears_each_key() {
        let mut keys = FlightKeys::default();
        for _ in 0..3 {
            keys.set(KeyCode::KeyD, true);
        }
        assert_eq!(
            keys.input(),
            GameplayVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0
            }
        );
        keys.set(KeyCode::KeyD, false);
        assert_eq!(keys.input(), GameplayVec3::ZERO);

        for key in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS] {
            keys.set(key, true);
            keys.set(key, false);
        }
        assert_eq!(keys.input(), GameplayVec3::ZERO);
    }

    #[test]
    fn flight_keys_update_authoritative_ship_position() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let before = bridge
            .session
            .world()
            .ship_position()
            .expect("visual session should have a ship");

        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);

        let after = bridge
            .session
            .world()
            .ship_position()
            .expect("visual session should retain its ship");
        assert_ne!(after, before);
    }

    #[test]
    fn releasing_flight_key_clears_acceleration_without_erasing_inertia() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        let moving = bridge
            .session
            .world()
            .ship_velocity()
            .expect("visual session should have a ship velocity");
        assert!(moving.x.is_finite() && moving.y.is_finite() && moving.z.is_finite());
        assert_ne!(moving, GameplayVec3::ZERO);

        bridge.handle_key(KeyCode::KeyW, ElementState::Released);
        assert_eq!(bridge.flight_keys.input(), GameplayVec3::ZERO);
        let residual = bridge
            .session
            .world()
            .ship_velocity()
            .expect("visual session should retain a ship velocity");
        assert!(residual.x.is_finite() && residual.y.is_finite() && residual.z.is_finite());
        assert_ne!(residual, GameplayVec3::ZERO);
    }

    #[test]
    fn s_then_w_can_invert_forward_velocity_after_releasing_s() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyS, ElementState::Pressed);
        for _ in 0..8 {
            bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        }
        bridge.handle_key(KeyCode::KeyS, ElementState::Released);
        let backward = bridge
            .session
            .world()
            .ship_velocity()
            .expect("ship velocity should exist");
        assert!(backward.z > 0.0);

        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        for _ in 0..30 {
            bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        }
        let forward = bridge
            .session
            .world()
            .ship_velocity()
            .expect("ship velocity should exist");
        assert!(forward.z < 0.0);
    }

    #[test]
    fn w_then_s_can_invert_backward_velocity_after_releasing_w() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        for _ in 0..12 {
            bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        }
        bridge.handle_key(KeyCode::KeyW, ElementState::Released);
        let forward = bridge
            .session
            .world()
            .ship_velocity()
            .expect("ship velocity should exist");
        assert!(forward.z < 0.0);

        bridge.handle_key(KeyCode::KeyS, ElementState::Pressed);
        for _ in 0..30 {
            bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        }
        let backward = bridge
            .session
            .world()
            .ship_velocity()
            .expect("ship velocity should exist");
        assert!(backward.z > 0.0);
    }

    #[test]
    fn arrow_keys_change_flight_camera_without_observation_transition() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let before = bridge
            .camera(1.0)
            .expect("camera should be valid")
            .position();

        bridge.handle_key(KeyCode::ArrowLeft, ElementState::Pressed);

        let after = bridge
            .camera(1.0)
            .expect("camera should remain valid")
            .position();
        assert_ne!(after, before);
        assert!(after.is_finite());
    }

    #[test]
    fn p_starts_the_configured_autopilot_route() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");

        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);

        assert!(matches!(
            bridge.session.world().autopilot_state(),
            gameplay::AutopilotState::InTransit { destination, .. } if *destination == DESTINATION
        ));
        assert!(matches!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::InProgress { destination, .. } if destination == DESTINATION
        ));
    }

    #[test]
    fn f_attacks_outside_landing_window_and_lands_only_when_valid() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");

        bridge.handle_key(KeyCode::KeyF, ElementState::Pressed);
        assert!(bridge
            .session
            .world()
            .events()
            .iter()
            .any(|event| matches!(event, GameplayEvent::AttackExecuted { .. })));
        assert!(!bridge.session.world().landing_available(DESTINATION));

        let landing_physics = PlanetPhysics {
            destination: DESTINATION,
            orbit_center: GameplayVec3::ZERO,
            orbit_radius: 5.0,
            orbit_start: GameplayVec3::ZERO,
            orbit_axis: GameplayVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            orbit_phase: 0.0,
            orbit_angular_speed: 0.0,
            surface_radius: game::render_bridge::PLANET_SURFACE_RADIUS,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        };
        assert!(bridge
            .session
            .world_mut()
            .configure_planet_physics(landing_physics));
        assert!(bridge.session.world().landing_available(DESTINATION));
        bridge.handle_key(KeyCode::KeyF, ElementState::Pressed);
        assert!(matches!(
            bridge.session.world().planet_flight_state(),
            gameplay::PlanetFlightState::Landing { planet } if planet == DESTINATION
        ));
    }

    #[test]
    fn every_planet_shows_the_landing_action_and_accepts_l_at_cruise_speed() {
        for destination in [
            DESTINATION,
            TESSARA_DESTINATION,
            KHARVEX_DESTINATION,
            NARETH_DESTINATION,
        ] {
            let mut bridge =
                VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
            let world = bridge.session.world();
            let physics = world
                .navigation()
                .planet_physics(destination)
                .expect("every visible planet should have landing physics");
            let planet_position = world
                .planet_position_at(destination, world.time().elapsed)
                .expect("planet position should be available");
            let landing_position = GameplayVec3 {
                x: planet_position.x + physics.collision_radius() + physics.atmosphere_height - 1.0,
                y: planet_position.y,
                z: planet_position.z,
            };
            let orientation = world.ship_orientation().unwrap();
            bridge
                .session
                .world_mut()
                .restore_ship_transform(
                    landing_position,
                    orientation,
                    GameplayVec3 {
                        x: 8.0,
                        y: 0.0,
                        z: 0.0,
                    },
                )
                .expect("ship should enter the landing corridor");
            bridge.pilot_mode = true;
            bridge.interior_mode = false;

            assert!(bridge.session.world().landing_available(destination));
            let action = bridge
                .contextual_action()
                .expect("landing action should appear near every planet");
            assert_eq!(action.key, "L");
            assert_eq!(action.object, VisualBridge::navigation_destination_label(destination));
            assert!(bridge.instructions_hint().contains("L ATERRIZAR"));
            assert!(bridge
                .quick_action_hints()
                .iter()
                .any(|value| value.contains("L ATERRIZAR")));

            bridge.handle_key(KeyCode::KeyL, ElementState::Pressed);
            assert!(matches!(
                bridge.session.world().planet_flight_state(),
                gameplay::PlanetFlightState::Landing { planet } if planet == destination
            ));
        }
    }

    #[test]
    fn haptic_event_cursor_does_not_reconsume_visual_events() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyF, ElementState::Pressed);
        bridge.poll_haptics();
        let cursor = bridge.haptic_event_cursor;
        assert!(cursor > 0);
        bridge.poll_haptics();
        assert_eq!(bridge.haptic_event_cursor, cursor);
    }

    #[test]
    fn awakening_can_start_once_from_the_visual_input() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let first = bridge
            .session
            .mission_progress(crate::el_despertar::EL_DESPERTAR)
            .expect("awakening should be registered");
        assert_eq!(
            first.state,
            gameplay::domains::mission::MissionState::Active
        );
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let started = bridge
            .session
            .world()
            .events()
            .iter()
            .filter(|event| {
                matches!(event, GameplayEvent::MissionStarted { mission, .. } if *mission == crate::el_despertar::EL_DESPERTAR)
            })
            .count();
        assert_eq!(started, 1);
    }

    #[test]
    fn awakening_visual_controls_follow_authoritative_gameplay_progress() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyR, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyY, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        assert!(bridge.feedback.contains("comandos=2"));
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        assert!(bridge.feedback.contains("comandos=2"));

        let progress = bridge
            .session
            .mission_progress(crate::el_despertar::EL_DESPERTAR)
            .expect("awakening should remain available");
        assert_eq!(progress.required_completed, progress.required_total);
        assert_eq!(
            progress.state,
            gameplay::domains::mission::MissionState::Completed
        );

        let snapshot =
            game::render_bridge::snapshot_from_world(bridge.session.world(), Default::default());
        assert_eq!(
            snapshot.overlay.hud.mission_name.as_deref(),
            Some("Ecos de la anomalía")
        );
        assert_eq!(
            snapshot.overlay.hud.mission_state,
            Some(render_core::HudMissionState::Available)
        );
        assert_eq!(snapshot.overlay.hud.mission_progress, Some((0, 4)));
    }

    #[test]
    fn mission_one_progresses_through_real_station_interactions() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);

        // Guidance selects the next authoritative interaction. Each I press
        // uses the same gameplay commands a player would use at the station.
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }

        let progress = bridge
            .session
            .mission_progress(crate::el_despertar::EL_DESPERTAR)
            .expect("awakening should be registered");
        assert_eq!(progress.required_completed, progress.required_total);
        assert_eq!(
            progress.state,
            gameplay::domains::mission::MissionState::Completed
        );
        assert!(!bridge.interior_mode);
        assert!(bridge
            .session
            .world()
            .electrical_panels()
            .all(|panel| panel.is_repaired()));
        assert!(bridge
            .session
            .world()
            .life_support_panels()
            .all(|panel| panel.is_repaired()));
    }

    #[test]
    fn mission_two_starts_with_real_visual_content_and_guided_travel() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);

        assert_eq!(
            bridge
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .expect("anomaly mission should be registered")
                .state,
            gameplay::domains::mission::MissionState::Active
        );
        assert!(bridge
            .session
            .world()
            .destination(crate::ecos_anomalia::SIGNAL_DESTINATION)
            .is_some());
        assert!(bridge
            .session
            .world()
            .destination(crate::ecos_anomalia::PLANET_DESTINATION)
            .is_some());

        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .expect("anomaly progress should exist")
                .required_completed,
            1
        );

        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        assert!(matches!(
            bridge.session.world().travel_state(),
            gameplay::TravelState::InProgress { destination, .. }
                if destination == crate::ecos_anomalia::PLANET_DESTINATION
        ));
    }

    #[test]
    fn mission_two_rejects_incompatible_existing_destination_without_mutating_it() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        bridge
            .session
            .register_destination(Destination {
                id: crate::ecos_anomalia::PLANET_DESTINATION,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3 {
                    x: 99.0,
                    y: 0.0,
                    z: 99.0,
                },
                travel_duration: Duration(1000),
                fuel_cost: 1,
                discovery: DiscoveryState::Known,
            })
            .expect("incompatible destination fixture should register");

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);

        assert_eq!(
            bridge
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .expect("anomaly mission should be registered")
                .state,
            gameplay::domains::mission::MissionState::Available
        );
        assert_eq!(
            bridge
                .session
                .world()
                .destination(crate::ecos_anomalia::PLANET_DESTINATION),
            Some(&Destination {
                id: crate::ecos_anomalia::PLANET_DESTINATION,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3 {
                    x: 99.0,
                    y: 0.0,
                    z: 99.0
                },
                travel_duration: Duration(1000),
                fuel_cost: 1,
                discovery: DiscoveryState::Known,
            })
        );
        assert!(bridge.feedback.contains("DESTINO INCOMPATIBLE"));
    }

    #[test]
    fn mission_two_completes_all_objectives_through_real_gameplay_systems() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);

        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);

        let progress = bridge
            .session
            .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
            .expect("anomaly mission should be registered");
        assert_eq!(progress.required_completed, 2);

        for _ in 0..20 {
            let Some(enemy_position) = bridge
                .session
                .world()
                .enemies()
                .find(|enemy| enemy.entity == bridge.enemy)
                .map(|enemy| enemy.position)
            else {
                break;
            };
            let Some(player_position) = bridge
                .session
                .world()
                .player(bridge.player)
                .map(|player| player.position)
            else {
                break;
            };
            let direction = GameplayVec3 {
                x: enemy_position.x - player_position.x,
                y: 0.0,
                z: enemy_position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            let direction = GameplayVec3 {
                x: direction.x / length,
                y: 0.0,
                z: direction.z / length,
            };
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction,
                delta: Duration(1_000),
            }]);
            bridge.session.update([GameplayCommand::AttackEnemy {
                player: bridge.player,
                enemy: bridge.enemy,
            }]);
            if bridge
                .session
                .world()
                .enemies()
                .find(|enemy| enemy.entity == bridge.enemy)
                .is_none_or(|enemy| enemy.is_defeated())
            {
                break;
            }
            bridge.session.update([GameplayCommand::AdvanceTime {
                delta: Duration(1_000),
            }]);
        }

        let reward = bridge
            .session
            .world()
            .collectibles()
            .find(|value| value.active && value.item == ItemId(3))
            .copied()
            .expect("Shooter defeat should generate ItemId(3) through RewardSystem");
        for _ in 0..20 {
            let Some(player_position) = bridge
                .session
                .world()
                .player(bridge.player)
                .map(|player| player.position)
            else {
                break;
            };
            let direction = GameplayVec3 {
                x: reward.position.x - player_position.x,
                y: 0.0,
                z: reward.position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            let direction = GameplayVec3 {
                x: direction.x / length,
                y: 0.0,
                z: direction.z / length,
            };
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction,
                delta: Duration(1_000),
            }]);
            let Some(updated_player) = bridge.session.world().player(bridge.player) else {
                break;
            };
            let dx = updated_player.position.x - reward.position.x;
            let dz = updated_player.position.z - reward.position.z;
            if dx * dx + dz * dz <= reward.interaction_range * reward.interaction_range {
                break;
            }
        }
        let collected = bridge.session.update([GameplayCommand::CollectItem {
            player: bridge.player,
            collectible: reward.entity,
            amount: 1,
        }]);
        assert!(
            matches!(
                collected.first().map(|value| &value.status),
                Some(gameplay::CommandStatus::Success { changed: true })
            ),
            "collect result: {:?}, player: {:?}, reward: {:?}",
            collected,
            bridge.session.world().player(bridge.player),
            bridge.session.world().collectible(reward.entity)
        );
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .expect("anomaly progress should exist")
                .state,
            gameplay::domains::mission::MissionState::Completed
        );
    }

    #[test]
    fn mission_three_completes_all_objectives_through_real_gameplay_systems() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);

        let enemy = bridge.enemy;
        for _ in 0..20 {
            let Some(enemy_position) = bridge
                .session
                .world()
                .enemies()
                .find(|value| value.entity == enemy)
                .map(|value| value.position)
            else {
                break;
            };
            let Some(player_position) = bridge
                .session
                .world()
                .player(bridge.player)
                .map(|value| value.position)
            else {
                break;
            };
            let direction = GameplayVec3 {
                x: enemy_position.x - player_position.x,
                y: 0.0,
                z: enemy_position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction: GameplayVec3 {
                    x: direction.x / length,
                    y: 0.0,
                    z: direction.z / length,
                },
                delta: Duration(1_000),
            }]);
            bridge.session.update([GameplayCommand::AttackEnemy {
                player: bridge.player,
                enemy,
            }]);
            if bridge
                .session
                .world()
                .enemies()
                .find(|value| value.entity == enemy)
                .is_none_or(|value| value.is_defeated())
            {
                break;
            }
            bridge.session.update([GameplayCommand::AdvanceTime {
                delta: Duration(1_000),
            }]);
        }
        let reward = bridge
            .session
            .world()
            .collectibles()
            .find(|value| value.active && value.item == ItemId(3))
            .copied()
            .expect("Mission2 reward should exist");
        for _ in 0..20 {
            let player_position = bridge
                .session
                .world()
                .player(bridge.player)
                .expect("player should exist")
                .position;
            let direction = GameplayVec3 {
                x: reward.position.x - player_position.x,
                y: 0.0,
                z: reward.position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction: GameplayVec3 {
                    x: direction.x / length,
                    y: 0.0,
                    z: direction.z / length,
                },
                delta: Duration(1_000),
            }]);
        }
        bridge.session.update([GameplayCommand::CollectItem {
            player: bridge.player,
            collectible: reward.entity,
            amount: 1,
        }]);

        let mission_two = bridge
            .session
            .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
            .expect("Mission2 should exist");
        assert_eq!(
            mission_two.state,
            gameplay::domains::mission::MissionState::Completed
        );
        let mission_three = bridge
            .session
            .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
            .expect("Mission3 should exist");
        assert_eq!(
            mission_three.state,
            gameplay::domains::mission::MissionState::Available
        );

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let mission_three = bridge
            .session
            .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
            .expect("Mission3 should exist after start");
        assert_eq!(
            mission_three.state,
            gameplay::domains::mission::MissionState::Active
        );
        let destination_count = bridge.session.world().destinations().count();
        let hazard_count = bridge.session.world().environmental_hazards().count();
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert_eq!(
            bridge.session.world().destinations().count(),
            destination_count
        );
        assert_eq!(
            bridge.session.world().environmental_hazards().count(),
            hazard_count
        );
        assert!(bridge
            .session
            .world()
            .destination(crate::tormenta_gravitacional::ESCAPE_DESTINATION)
            .is_some());
        assert!(bridge
            .session
            .world()
            .environmental_hazard(crate::tormenta_gravitacional::HAZARD)
            .is_some());

        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
                .expect("Mission3 progress should exist")
                .required_completed,
            1
        );

        bridge.handle_key(KeyCode::KeyE, ElementState::Pressed);
        let engineering_position = bridge
            .session
            .world()
            .interactable(bridge.engineering_station)
            .expect("engineering station should exist")
            .position;
        for _ in 0..20 {
            let player_position = bridge
                .session
                .world()
                .player(bridge.player)
                .expect("player should exist")
                .position;
            let direction = GameplayVec3 {
                x: engineering_position.x - player_position.x,
                y: 0.0,
                z: engineering_position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            if length <= 3.0 {
                break;
            }
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction: GameplayVec3 {
                    x: direction.x / length,
                    y: 0.0,
                    z: direction.z / length,
                },
                delta: Duration(1_000),
            }]);
        }
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
                .expect("Mission3 progress should exist")
                .required_completed,
            2
        );

        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
                .expect("Mission3 progress should exist")
                .state,
            gameplay::domains::mission::MissionState::Completed,
            "progress={:?}, travel={:?}, position={:?}, events={:?}",
            bridge
                .session
                .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL),
            bridge.session.world().travel_state(),
            bridge.session.world().ship_position(),
            bridge.session.world().events()
        );
    }

    fn completed_campaign_bridge() -> VisualBridge {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        for _ in 0..20 {
            let enemy_position = bridge
                .session
                .world()
                .enemies()
                .find(|value| value.entity == bridge.enemy)
                .map(|value| value.position)
                .expect("Mission2 shooter should exist");
            let player_position = bridge
                .session
                .world()
                .player(bridge.player)
                .expect("player should exist")
                .position;
            let direction = GameplayVec3 {
                x: enemy_position.x - player_position.x,
                y: 0.0,
                z: enemy_position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction: GameplayVec3 {
                    x: direction.x / length,
                    y: 0.0,
                    z: direction.z / length,
                },
                delta: Duration(1_000),
            }]);
            bridge.session.update([GameplayCommand::AttackEnemy {
                player: bridge.player,
                enemy: bridge.enemy,
            }]);
            if bridge
                .session
                .world()
                .enemies()
                .find(|value| value.entity == bridge.enemy)
                .is_none_or(|value| value.is_defeated())
            {
                break;
            }
            bridge.session.update([GameplayCommand::AdvanceTime {
                delta: Duration(1_000),
            }]);
        }
        let reward = bridge
            .session
            .world()
            .collectibles()
            .find(|value| value.active && value.item == ItemId(3))
            .copied()
            .expect("Mission2 reward should exist");
        bridge.session.update([GameplayCommand::CollectItem {
            player: bridge.player,
            collectible: reward.entity,
            amount: 1,
        }]);

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        bridge.handle_key(KeyCode::KeyE, ElementState::Pressed);
        let engineering_position = bridge
            .session
            .world()
            .interactable(bridge.engineering_station)
            .expect("engineering station should exist")
            .position;
        for _ in 0..20 {
            let player_position = bridge
                .session
                .world()
                .player(bridge.player)
                .expect("player should exist")
                .position;
            let direction = GameplayVec3 {
                x: engineering_position.x - player_position.x,
                y: 0.0,
                z: engineering_position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            if length <= 3.0 {
                break;
            }
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction: GameplayVec3 {
                    x: direction.x / length,
                    y: 0.0,
                    z: direction.z / length,
                },
                delta: Duration(1_000),
            }]);
        }
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyP, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        bridge
            .session
            .update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);

        let mission_three = bridge
            .session
            .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
            .expect("Mission3 should exist");
        assert_eq!(
            mission_three.state,
            gameplay::domains::mission::MissionState::Completed
        );
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE)
                .expect("Mission4 should exist")
                .state,
            gameplay::domains::mission::MissionState::Available
        );

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let shielded = bridge.enemy;
        assert!(bridge
            .session
            .world()
            .enemies()
            .any(|value| value.entity == shielded
                && value.archetype == Some(EnemyArchetype::Shielded)
                && value.active));
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE)
                .expect("Mission4 should exist")
                .state,
            gameplay::domains::mission::MissionState::Active
        );
        let shielded_count = bridge
            .session
            .world()
            .enemies()
            .filter(|value| value.archetype == Some(EnemyArchetype::Shielded) && value.active)
            .count();
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .world()
                .enemies()
                .filter(|value| value.archetype == Some(EnemyArchetype::Shielded) && value.active)
                .count(),
            shielded_count
        );

        bridge.enter_pilot();
        assert!(bridge.pilot_mode);
        bridge.handle_key(KeyCode::KeyF, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE)
                .expect("Mission4 progress should exist")
                .required_completed,
            1
        );
        bridge.handle_key(KeyCode::KeyV, ElementState::Pressed);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        }]);
        bridge.handle_key(KeyCode::KeyF, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE)
                .expect("Mission4 progress should exist")
                .required_completed,
            3
        );
        bridge.handle_key(KeyCode::KeyE, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE)
                .expect("Mission4 progress should exist")
                .state,
            gameplay::domains::mission::MissionState::Completed
        );

        let mission_five = bridge
            .session
            .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
            .expect("Mission5 should exist");
        assert_eq!(
            mission_five.state,
            gameplay::domains::mission::MissionState::Available
        );
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let mission_five = bridge
            .session
            .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
            .expect("Mission5 should exist after start");
        assert_eq!(
            mission_five.state,
            gameplay::domains::mission::MissionState::Active
        );
        assert!(bridge
            .session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_some());
        let shooter_count = bridge
            .session
            .world()
            .enemies()
            .filter(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shooter)
                    && !value.is_defeated()
            })
            .count();
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .world()
                .enemies()
                .filter(|value| {
                    value.active
                        && value.archetype == Some(EnemyArchetype::Shooter)
                        && !value.is_defeated()
                })
                .count(),
            shooter_count
        );
        bridge.handle_key(KeyCode::KeyT, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyT, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .world()
                .enemies()
                .filter(|value| {
                    value.active
                        && value.archetype == Some(EnemyArchetype::Shooter)
                        && !value.is_defeated()
                })
                .count(),
            shooter_count
        );

        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        bridge.handle_key(KeyCode::Space, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyQ, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .expect("Mission5 progress should exist")
                .required_completed,
            1
        );

        bridge.handle_key(KeyCode::KeyC, ElementState::Pressed);
        let shooter = bridge.enemy;
        bridge.issue([GameplayCommand::Interact {
            player: bridge.player,
            target: bridge.pilot_station,
        }]);
        bridge.enter_pilot();
        assert!(bridge.pilot_mode);
        for _ in 0..30 {
            let Some(enemy_position) = bridge
                .session
                .world()
                .enemies()
                .find(|value| value.entity == shooter)
                .map(|value| value.position)
            else {
                break;
            };
            let player_position = bridge
                .session
                .world()
                .player(bridge.player)
                .expect("player should exist")
                .position;
            let direction = GameplayVec3 {
                x: enemy_position.x - player_position.x,
                y: 0.0,
                z: enemy_position.z - player_position.z,
            };
            let length = (direction.x * direction.x + direction.z * direction.z).sqrt();
            bridge.session.update([GameplayCommand::MovePlayer {
                player: bridge.player,
                direction: GameplayVec3 {
                    x: direction.x / length,
                    y: 0.0,
                    z: direction.z / length,
                },
                delta: Duration(1_000),
            }]);
            bridge.handle_key(KeyCode::KeyF, ElementState::Pressed);
            if bridge
                .session
                .world()
                .enemies()
                .find(|value| value.entity == shooter)
                .is_none_or(|value| value.is_defeated())
            {
                break;
            }
            bridge.session.update([GameplayCommand::AdvanceTime {
                delta: Duration(1_000),
            }]);
        }
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .expect("Mission5 progress should exist")
                .required_completed,
            2
        );
        bridge.handle_key(KeyCode::KeyE, ElementState::Pressed);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .expect("Mission5 progress should exist")
                .state,
            gameplay::domains::mission::MissionState::Completed
        );
        bridge
    }

    #[test]
    fn campaign_missions_one_through_five_complete_end_to_end() {
        let _ = completed_campaign_bridge();
    }

    #[test]
    fn campaign_missions_one_through_five_persist_after_mission_five() {
        let bridge = completed_campaign_bridge();
        let player = bridge.player;
        let mission_ids = [
            crate::el_despertar::EL_DESPERTAR,
            crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA,
            crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL,
            crate::la_defensa_imposible::LA_DEFENSA_IMPOSIBLE,
            crate::cooperacion::PROGRESION_COOPERATIVA,
        ];
        let mission_state = |session: &ProjectEchoSession, mission| {
            session
                .mission_progress(mission)
                .expect("campaign mission should exist")
        };
        let before_inventory = bridge
            .session
            .world()
            .inventory(player)
            .cloned()
            .expect("campaign player inventory should exist");
        let before_item_three = before_inventory.quantity(ItemId(3));
        assert!(
            before_item_three > 0,
            "Mission5 should preserve its ItemId(3) reward"
        );
        let defeated_enemies: Vec<_> = bridge
            .session
            .world()
            .enemies()
            .filter(|enemy| enemy.is_defeated())
            .map(|enemy| enemy.entity)
            .collect();
        let consumed_collectibles: Vec<_> = bridge
            .session
            .world()
            .collectibles()
            .filter(|collectible| !collectible.active || collectible.amount == 0)
            .map(|collectible| (collectible.entity, collectible.active, collectible.amount))
            .collect();
        let consumed_collectible = consumed_collectibles
            .first()
            .map(|value| value.0)
            .expect("campaign should have a consumed reward collectible");
        let before_missions: Vec<_> = mission_ids
            .iter()
            .map(|mission| mission_state(&bridge.session, *mission))
            .collect();
        assert!(before_missions.iter().all(|progress| {
            progress.state == gameplay::domains::mission::MissionState::Completed
                && progress.required_completed == progress.required_total
        }));
        let before_ship_position = bridge.session.world().ship_position();
        let saved = bridge.session.save_state();
        let serialized = saved.serialize();
        let decoded = gameplay::GameplaySaveState::deserialize(&serialized)
            .expect("completed campaign save should deserialize");
        let mut restored = ProjectEchoSession::new();
        restored
            .restore_state(decoded)
            .expect("completed campaign save should restore into a new session");

        let restored_missions: Vec<_> = mission_ids
            .iter()
            .map(|mission| mission_state(&restored, *mission))
            .collect();
        assert_eq!(restored_missions, before_missions);
        assert_eq!(
            mission_state(&restored, crate::cooperacion::PROGRESION_COOPERATIVA).state,
            gameplay::domains::mission::MissionState::Completed
        );
        assert_eq!(
            mission_state(&restored, crate::cooperacion::PROGRESION_COOPERATIVA).required_completed,
            mission_state(&restored, crate::cooperacion::PROGRESION_COOPERATIVA).required_total
        );
        assert_eq!(restored.world().inventory(player), Some(&before_inventory));
        assert_eq!(
            restored
                .world()
                .inventory(player)
                .expect("restored inventory should exist")
                .quantity(ItemId(3)),
            before_item_three
        );
        assert_eq!(restored.world().ship_position(), before_ship_position);
        for enemy in defeated_enemies {
            assert!(restored
                .world()
                .combatant(enemy)
                .is_some_and(|value| value.is_defeated() && !value.active));
        }
        for (collectible, active, amount) in consumed_collectibles {
            assert_eq!(
                restored
                    .world()
                    .collectible(collectible)
                    .map(|value| (value.active, value.amount)),
                Some((active, amount))
            );
        }

        let restored_before_repeat = restored.save_state().serialize();
        let repeated_collect = restored.update([GameplayCommand::CollectItem {
            player,
            collectible: consumed_collectible,
            amount: 1,
        }]);
        assert!(matches!(
            repeated_collect.first().map(|result| &result.status),
            Some(gameplay::CommandStatus::Rejected { .. })
        ));
        assert_eq!(restored.save_state().serialize(), restored_before_repeat);
        assert_eq!(
            mission_state(&restored, crate::cooperacion::PROGRESION_COOPERATIVA).state,
            gameplay::domains::mission::MissionState::Completed
        );

        let continuation_results =
            restored.update([GameplayCommand::AdvanceTime { delta: Duration(1) }]);
        assert!(matches!(
            continuation_results.first().map(|result| &result.status),
            Some(gameplay::CommandStatus::Success { .. })
        ));
        let continued = restored.save_state().serialize();
        let mut continued_restored = ProjectEchoSession::new();
        continued_restored
            .restore_state(
                gameplay::GameplaySaveState::deserialize(&continued)
                    .expect("post-campaign continuation save should deserialize"),
            )
            .expect("post-campaign continuation save should restore");
        assert_eq!(continued_restored.save_state().serialize(), continued);
        assert_eq!(
            continued_restored.world().inventory(player),
            Some(&before_inventory)
        );
        assert_eq!(
            mission_state(
                &continued_restored,
                crate::cooperacion::PROGRESION_COOPERATIVA
            )
            .state,
            gameplay::domains::mission::MissionState::Completed
        );
    }

    #[test]
    fn visual_start_key_starts_available_successor_without_auto_starting_it() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyR, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyY, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);

        assert_eq!(
            bridge
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .expect("awakening should be registered")
                .state,
            gameplay::domains::mission::MissionState::Completed
        );
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .expect("anomaly mission should be registered")
                .state,
            gameplay::domains::mission::MissionState::Available
        );

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);

        assert_eq!(
            bridge
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .expect("anomaly mission should remain registered")
                .state,
            gameplay::domains::mission::MissionState::Active
        );
    }

    #[test]
    fn derelict_weapon_can_be_collected_equipped_and_restored() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let player = bridge.player;
        let derelict = DerelictId::new(9_010);

        for _ in 0..4 {
            bridge.session.update([
                GameplayCommand::AccelerateShip {
                    player,
                    pilot_station: bridge.pilot_station,
                    input: GameplayVec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                },
                GameplayCommand::AdvanceTime {
                    delta: Duration(1_000),
                },
            ]);
        }
        assert!(bridge
            .session
            .world()
            .ship_position()
            .is_some_and(|value| value.x > 35.0));

        bridge.guided_interaction();
        assert_eq!(
            bridge
                .session
                .world()
                .derelict(derelict)
                .map(|value| value.state),
            Some(gameplay::DerelictState::Exploring)
        );
        assert_eq!(
            bridge.contextual_action().map(|value| value.object),
            Some("ARMA DE ENERGIA".to_owned())
        );
        assert!(bridge
            .session
            .world()
            .collectibles()
            .any(|value| value.active && value.item == gameplay::ENERGY_WEAPON_ITEM));

        bridge.guided_interaction();
        assert_eq!(
            bridge
                .session
                .world()
                .inventory(player)
                .expect("player inventory should exist")
                .quantity(gameplay::ENERGY_WEAPON_ITEM),
            1
        );
        assert!(bridge.contextual_action().is_none());
        assert!(matches!(
            bridge
                .session
                .update([GameplayCommand::EquipPlayerWeaponItem {
                    player,
                    item: gameplay::ENERGY_WEAPON_ITEM,
                }])[0]
                .status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert_eq!(
            bridge
                .session
                .world()
                .equipped_weapon(player)
                .map(|value| value.attack_type),
            Some(gameplay::AttackType::Energy)
        );
        bridge.session.update([GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(3),
            amount: 1,
        }]);
        let enemy_health = bridge
            .session
            .world()
            .combatant(bridge.enemy)
            .unwrap()
            .current_health;
        assert!(matches!(
            bridge.session.update([GameplayCommand::AttackEnemy {
                player,
                enemy: bridge.enemy,
            }])[0]
                .status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert!(
            bridge
                .session
                .world()
                .combatant(bridge.enemy)
                .unwrap()
                .current_health
                < enemy_health
        );
        assert!(matches!(
            bridge
                .session
                .update([GameplayCommand::EquipPlayerWeaponItem {
                    player,
                    item: gameplay::PLASMA_WEAPON_ITEM,
                }])[0]
                .status,
            gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::WeaponNotOwned
            }
        ));
        let energy =
            gameplay::weapon_for_item(gameplay::ENERGY_WEAPON_ITEM).expect("energy weapon");
        let plasma =
            gameplay::weapon_for_item(gameplay::PLASMA_WEAPON_ITEM).expect("plasma weapon");
        assert_ne!(
            (energy.damage, energy.range, energy.cooldown),
            (plasma.damage, plasma.range, plasma.cooldown)
        );

        let save = bridge.session.save_state();
        let serialized = save.serialize();
        let mut restored = VisualBridge::new().expect("restored visual bridge should initialize");
        restored
            .session
            .restore_state(save)
            .expect("weapon state should restore");
        assert!(serialized.contains("item=",) || serialized.contains("inventory|"));
        assert_eq!(
            restored
                .session
                .world()
                .inventory(player)
                .unwrap()
                .quantity(gameplay::ENERGY_WEAPON_ITEM),
            1
        );
        assert_eq!(
            restored
                .session
                .world()
                .equipped_weapon(player)
                .map(|value| value.attack_type),
            Some(gameplay::AttackType::Energy)
        );
        assert!(!restored
            .session
            .world()
            .collectibles()
            .any(|value| value.active && value.item == gameplay::ENERGY_WEAPON_ITEM));
    }

    #[test]
    fn resource_derelicts_publish_collectible_resources_and_persist_consumption() {
        let mut bridge = VisualBridge::new().expect("resource scenario should initialize");
        bridge.session.update([GameplayCommand::RegisterDerelict {
            derelict: Derelict::new(DerelictId::new(9_020), GameplayVec3::ZERO, 3.5).with_rewards(
                vec![DerelictReward {
                    item: gameplay::FUEL_RESOURCE_ITEM,
                    amount: 4,
                }],
                RewardDistributionPolicy::Shared,
            ),
        }]);
        bridge.guided_interaction();
        let fuel_collectible = bridge
            .session
            .world()
            .collectibles()
            .find(|value| value.item == gameplay::FUEL_RESOURCE_ITEM)
            .copied()
            .expect("fuel derelict should create a collectible");
        assert!(fuel_collectible.active);
        let collect_result = bridge.session.update([GameplayCommand::CollectItem {
            player: bridge.player,
            collectible: fuel_collectible.entity,
            amount: fuel_collectible.amount,
        }]);
        assert!(matches!(
            collect_result.first().map(|value| value.status),
            Some(gameplay::CommandStatus::Success { changed: true })
        ));
        assert!(
            !bridge
                .session
                .world()
                .collectible(fuel_collectible.entity)
                .unwrap()
                .active
        );
        let fuel_before = bridge
            .session
            .world()
            .ship_fuel(bridge.ship)
            .unwrap()
            .current();
        bridge.session.update([GameplayCommand::AddInventoryItem {
            player: bridge.player,
            item: gameplay::FUEL_RESOURCE_ITEM,
            amount: 1,
        }]);
        let result = bridge.session.update([GameplayCommand::UseFuelResource {
            player: bridge.player,
            ship: bridge.ship,
            item: gameplay::FUEL_RESOURCE_ITEM,
            amount: 1,
        }]);
        assert!(matches!(
            result.first().map(|value| value.status),
            Some(gameplay::CommandStatus::Success { changed: true })
        ));
        assert_eq!(
            bridge
                .session
                .world()
                .ship_fuel(bridge.ship)
                .unwrap()
                .current(),
            fuel_before + 1
        );
        let restored = bridge.session.save_state().serialize();
        assert!(restored.contains("fuel|"));
        assert!(restored.contains("collectible|"));
    }

    #[test]
    fn textual_save_restores_life_support_allocations_and_old_saves_default_them() {
        let mut bridge =
            VisualBridge::new_with_mode(true).expect("interior scenario should initialize");
        bridge.restore_life_support();
        let before = bridge
            .session
            .world()
            .life_support_panel(bridge.life_support_panel)
            .unwrap()
            .progress();
        assert!(before.repaired);
        let serialized = bridge.session.save_state().serialize();
        assert!(serialized.lines().any(|line| line.starts_with("oxygen|")));

        let decoded = gameplay::GameplaySaveState::deserialize(&serialized)
            .expect("oxygen save should parse");
        let mut restored =
            VisualBridge::new_with_mode(true).expect("restored interior should initialize");
        restored
            .session
            .restore_state(decoded)
            .expect("oxygen save should restore");
        assert_eq!(
            restored
                .session
                .world()
                .life_support_panel(restored.life_support_panel)
                .unwrap()
                .progress(),
            before
        );
        let repeated = restored.session.save_state().serialize();
        let repeated_decoded = gameplay::GameplaySaveState::deserialize(&repeated)
            .expect("repeated save should parse");
        restored
            .session
            .restore_state(repeated_decoded)
            .expect("repeated oxygen save should restore");
        assert_eq!(
            restored
                .session
                .world()
                .life_support_panel(restored.life_support_panel)
                .unwrap()
                .progress(),
            before
        );

        let old_text = serialized
            .lines()
            .filter(|line| !line.starts_with("oxygen|"))
            .collect::<Vec<_>>()
            .join("\n");
        let old_decoded =
            gameplay::GameplaySaveState::deserialize(&old_text).expect("old save should parse");
        let mut old_restored =
            VisualBridge::new_with_mode(true).expect("old restored interior should initialize");
        old_restored
            .session
            .restore_state(old_decoded)
            .expect("old save should restore");
        assert_eq!(
            old_restored
                .session
                .world()
                .life_support_panel(old_restored.life_support_panel)
                .unwrap()
                .allocated_total(),
            0
        );
        let invalid_text = serialized
            .lines()
            .map(|line| {
                if line.starts_with("oxygen|") {
                    "oxygen|6|1:99,2:2"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let invalid_decoded = gameplay::GameplaySaveState::deserialize(&invalid_text)
            .expect("invalid oxygen record should parse before domain validation");
        let mut invalid_restored =
            VisualBridge::new_with_mode(true).expect("invalid restored interior should initialize");
        assert!(invalid_restored
            .session
            .restore_state(invalid_decoded)
            .is_err());
    }

    #[test]
    fn collected_supplies_repair_the_ship_once_and_invalid_repairs_are_atomic() {
        let mut bridge = VisualBridge::new().expect("repair scenario should initialize");
        bridge.session.update([GameplayCommand::RegisterDerelict {
            derelict: Derelict::new(DerelictId::new(9_021), GameplayVec3::ZERO, 3.5).with_rewards(
                vec![DerelictReward {
                    item: ItemId(1),
                    amount: 1,
                }],
                RewardDistributionPolicy::Shared,
            ),
        }]);
        bridge.guided_interaction();
        bridge.guided_interaction();
        assert_eq!(
            bridge
                .session
                .world()
                .inventory(bridge.player)
                .unwrap()
                .quantity(ItemId(1)),
            1
        );
        bridge.session.update([GameplayCommand::DamageShip {
            ship: bridge.ship,
            amount: 20,
        }]);
        let before = bridge
            .session
            .world()
            .inventory(bridge.player)
            .unwrap()
            .quantity(ItemId(1));
        bridge.session.update([GameplayCommand::Interact {
            player: bridge.player,
            target: bridge.engineering_station,
        }]);
        let repaired = bridge
            .session
            .update([GameplayCommand::RepairShipAtStation {
                player: bridge.player,
                ship: bridge.ship,
                station: bridge.engineering_station,
            }]);
        assert!(matches!(
            repaired.first().map(|value| value.status),
            Some(gameplay::CommandStatus::Success { changed: true })
        ));
        assert_eq!(
            bridge
                .session
                .world()
                .inventory(bridge.player)
                .unwrap()
                .quantity(ItemId(1)),
            before - 1
        );
        let after = bridge
            .session
            .world()
            .ship_integrity(bridge.ship)
            .unwrap()
            .current();
        assert!(after > 80);
        let rejected = bridge
            .session
            .update([GameplayCommand::RepairShipAtStation {
                player: bridge.player,
                ship: bridge.ship,
                station: bridge.engineering_station,
            }]);
        assert!(matches!(
            rejected.first().map(|value| value.status),
            Some(gameplay::CommandStatus::Rejected { .. })
        ));
        assert_eq!(
            bridge
                .session
                .world()
                .inventory(bridge.player)
                .unwrap()
                .quantity(ItemId(1)),
            0
        );
    }

    #[test]
    fn visual_bridge_sends_one_test_asset_request_and_receives_ready() {
        let mut bridge = VisualBridge::new().expect("visual bridge should initialize");
        let (client, mut host) = visual_asset_channel();
        bridge.attach_visual_asset_client(client);
        let path = std::env::temp_dir().join(format!(
            "astral_echo_visual_asset_test_{}.obj",
            std::process::id()
        ));
        std::fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").expect("write test OBJ");
        let before_tick = bridge.session.world().time().tick;
        assert!(bridge.request_visual_asset(
            "fixture.visual_bridge",
            &path,
            MeshHandle::new(30_020),
            MaterialHandle::new(30_020),
        ));
        assert!(!bridge.request_visual_asset(
            "fixture.visual_bridge",
            &path,
            MeshHandle::new(30_020),
            MaterialHandle::new(30_020),
        ));
        assert_eq!(
            bridge.visual_asset_states.get("fixture.visual_bridge"),
            Some(&VisualAssetState::Loading)
        );
        let mut registry = DeferredMeshRegistry::new();
        let mut scene = render_core::SnapshotScene::default();
        host.poll(&mut registry, Some(&mut scene));
        bridge.poll_visual_asset_events();
        for _ in 0..10_000 {
            host.poll(&mut registry, Some(&mut scene));
            bridge.poll_visual_asset_events();
            if bridge.visual_asset_states.get("fixture.visual_bridge")
                == Some(&VisualAssetState::Ready)
            {
                assert_eq!(bridge.session.world().time().tick, before_tick);
                assert!(scene
                    .shared_mesh(MeshHandle::new(30_020), MaterialHandle::new(30_020))
                    .is_ok());
                std::fs::remove_file(path).expect("remove test OBJ");
                return;
            }
            std::thread::yield_now();
        }
        std::fs::remove_file(path).expect("remove test OBJ");
        panic!("VisualBridge did not receive Ready");
    }

    #[test]
    #[ignore = "loads the production OBJ files; run for render batching diagnostics"]
    fn runtime_models_are_batched_by_material() {
        let assets_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models");
        let expected_max_parts = [1usize, 1, 24, 11, 4, 15];
        let mut scene = render_core::SnapshotScene::default();

        for (index, (definition, expected_max)) in crate::model_registry::registered_models()
            .into_iter()
            .zip(expected_max_parts)
            .enumerate()
        {
            let first_id = 700_000 + index as u32 * 20_000;
            let parts = render_core::register_textured_model(
                &mut scene,
                assets_root.join(definition.path),
                first_id,
                first_id,
            )
            .unwrap_or_else(|error| panic!("failed to batch {}: {error}", definition.path));
            println!("{}: {} render parts", definition.path, parts.len());
            assert!(
                parts.len() <= expected_max,
                "{} produced {} parts; expected at most {expected_max}",
                definition.path,
                parts.len()
            );
        }
    }
}
