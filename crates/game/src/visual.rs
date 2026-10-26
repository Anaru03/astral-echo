use std::cell::RefCell;
use std::rc::Rc;

use engine::{Duration, EntityId, Vec3 as GameplayVec3};
use game::ProjectEchoSession;
use gameplay::types::ItemId;
use gameplay::{
    AttackType, Destination, DestinationId, DestinationType, DiscoveryState, EnemyArchetype,
    EnemyConfiguration, EncounterId, GameplayCommand, GameplayEvent, NavigationRoute,
    NavigationRouteState, SpatialEncounter, SpatialEncounterKind, StationType, WeaponAmmunition,
    WeaponConfiguration, PlanetPhysics,
};
use render_core::{LightingMode, MaterialInfo, VisualAssets};
use render_present::{
    run_hot_swap_window_with_input_and_scene, ElementState, GraphicsDebugConfig, KeyCode, Preset,
    WindowConfig,
};

const ORIGIN: DestinationId = DestinationId::new(9_001);
const DESTINATION: DestinationId = DestinationId::new(9_002);
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
}

impl FlightKeys {
    fn set(&mut self, key: KeyCode, active: bool) -> bool {
        let slot = match key {
            KeyCode::KeyW => &mut self.forward,
            KeyCode::KeyS => &mut self.backward,
            KeyCode::KeyA => &mut self.left,
            KeyCode::KeyD => &mut self.right,
            _ => return false,
        };
        *slot = active;
        true
    }

    fn input(self) -> GameplayVec3 {
        GameplayVec3 {
            x: if self.right { 1.0 } else { 0.0 } - if self.left { 1.0 } else { 0.0 },
            y: 0.0,
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
}

struct VisualBridge {
    session: ProjectEchoSession,
    player: EntityId,
    ship: EntityId,
    pilot_station: EntityId,
    enemy: EntityId,
    feedback: String,
    observation: ObservationTransition,
    flight_keys: FlightKeys,
}

impl VisualBridge {
    fn new() -> Result<Self, String> {
        let mut session = ProjectEchoSession::new();
        let player = session.ids.first_player;
        let ship = session.ids.first_ship;
        session
            .register_destination(Destination {
                id: ORIGIN,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3::ZERO,
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
                distance: 49.0,
                duration: Duration(1_000),
                fuel_cost: 1,
                state: NavigationRouteState::Available,
            },
        }]);
        session.update([GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: DESTINATION,
                to: ORIGIN,
                distance: 49.0,
                duration: Duration(1_000),
                fuel_cost: 1,
                state: NavigationRouteState::Available,
            },
        }]);
        if !session.world_mut().configure_planet_physics(PlanetPhysics {
            destination: DESTINATION,
            orbit_center: GameplayVec3 {
                x: 0.0,
                y: 0.0,
                z: -18.0,
            },
            orbit_radius: game::render_bridge::PLANET_ORBIT_RADIUS,
            orbit_angular_speed: game::render_bridge::PLANET_ORBIT_ANGULAR_SPEED_RAD_PER_SECOND,
            surface_radius: game::render_bridge::PLANET_SURFACE_RADIUS,
            ship_radius: 1.0,
            gravity_influence_radius: 24.0,
            gravitational_parameter: 80.0,
            atmosphere_height: 4.0,
        }) {
            return Err("configurar la fÃ­sica planetaria visual".to_owned());
        }
        let pilot_station = session
            .create_station(GameplayVec3::ZERO, 3.0, StationType::Piloting)
            .map_err(|error| format!("crear estación visual: {error:?}"))?;
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
                        configuration: EnemyConfiguration::for_archetype(
                            EnemyArchetype::Shooter,
                            GameplayVec3 {
                                x: 18.0,
                                y: 0.0,
                                z: -18.0,
                            },
                        ),
                    },
                    GameplayVec3::ZERO,
                    4.0,
                ),
            },
            GameplayCommand::ActivateSpatialEncounter {
                encounter: ENCOUNTER,
            },
        ]);
        let enemy = session
            .world()
            .events()
            .iter()
            .find_map(|event| match event {
                GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
                _ => None,
            })
            .ok_or_else(|| "el escenario visual no creó el enemigo".to_owned())?;
        session.update([GameplayCommand::Interact {
            player,
            target: pilot_station,
        }]);
        session.update([GameplayCommand::SetShipControlActive {
            player,
            pilot_station,
            active: true,
        }]);
        Ok(Self {
            session,
            player,
            ship,
            pilot_station,
            enemy,
            feedback: "ESCENA INICIALIZADA".to_owned(),
            observation: ObservationTransition::default(),
            flight_keys: FlightKeys::default(),
        })
    }

    fn issue(&mut self, commands: impl IntoIterator<Item = GameplayCommand>) {
        let results = self.session.update(commands);
        let status = results
            .last()
            .map(|result| format!("{:?}", result.status))
            .unwrap_or_else(|| "sin resultado".to_owned());
        let event = self
            .session
            .world()
            .events()
            .iter()
            .last()
            .map(|value| format!("{:?}", value))
            .unwrap_or_else(|| "sin evento".to_owned());
        self.feedback = format!("{status}; {event}");
        println!("visual gameplay: {}", self.feedback);
    }

    fn handle_key(&mut self, key: KeyCode, state: ElementState) {
        let player = self.player;
        let ship = self.ship;
        let enemy = self.enemy;
        let pilot_station = self.pilot_station;
        if self.flight_keys.set(key, state == ElementState::Pressed) {
            self.accelerate_ship(player, pilot_station, self.flight_keys.input());
            return;
        }
        if state != ElementState::Pressed {
            return;
        }
        match key {
            KeyCode::Space => self.issue([GameplayCommand::AdvanceTime {
                delta: Duration(100),
            }]),
            KeyCode::KeyQ => self.issue([GameplayCommand::ScanSignals { player, ship }]),
            KeyCode::KeyF => {
                if self.session.world().landing_available(DESTINATION) {
                    if self.session.world_mut().request_planet_landing(DESTINATION) {
                        self.feedback = "ATERRIZAJE CONFIRMADO".to_owned();
                        println!("visual gameplay: {}", self.feedback);
                    }
                } else {
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
            KeyCode::KeyP => self.toggle_flight(player, pilot_station),
            KeyCode::KeyO => self.toggle_observation(),
            KeyCode::ArrowLeft => self.observation.adjust_orbit(-OBSERVATION_AZIMUTH_STEP, 0.0),
            KeyCode::ArrowRight => self.observation.adjust_orbit(OBSERVATION_AZIMUTH_STEP, 0.0),
            KeyCode::ArrowUp => self.observation.adjust_orbit(0.0, OBSERVATION_ELEVATION_STEP),
            KeyCode::ArrowDown => self.observation.adjust_orbit(0.0, -OBSERVATION_ELEVATION_STEP),
            _ => {}
        }
    }

    fn accelerate_ship(&mut self, player: EntityId, pilot_station: EntityId, input: GameplayVec3) {
        self.issue([
            GameplayCommand::AccelerateShip {
                player,
                pilot_station,
                input,
            },
            GameplayCommand::AdvanceTime {
                delta: Duration(100),
            },
        ]);
    }

    fn toggle_observation(&mut self) {
        let now = self.session.world().time().elapsed.0;
        self.observation.toggle(now);
    }

    fn toggle_flight(&mut self, player: EntityId, pilot_station: EntityId) {
        let (origin, destination) = match self.session.world().travel_state() {
            gameplay::TravelState::Completed { destination } if destination == DESTINATION => {
                (DESTINATION, ORIGIN)
            }
            _ => (ORIGIN, DESTINATION),
        };
        self.issue([
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
        ]);
    }

    fn camera(&self, aspect: f32) -> Result<render_core::Camera, render_core::CameraError> {
        let world = self.session.world();
        game::render_bridge::observation_camera(
            world,
            aspect,
            self.observation.progress(world.time().elapsed.0),
            self.observation.azimuth,
            self.observation.elevation,
        )
    }
}

pub fn run() -> Result<(), String> {
    let runtime = Rc::new(RefCell::new(VisualBridge::new()?));
    let snapshot_runtime = Rc::clone(&runtime);
    let input_runtime = Rc::clone(&runtime);
    let config = WindowConfig {
        title: "Astral Echo — CPU propio".into(),
        preset: Preset::Low,
        raster_controls: false,
        ..Default::default()
    };
    run_hot_swap_window_with_input_and_scene(
        config,
        Box::new(move |width, height| {
            let bridge = snapshot_runtime.borrow();
            let camera = bridge
                .camera(width as f32 / height.max(1) as f32)
                .map_err(|error| format!("cámara visual: {error:?}"))?;
            Ok(game::render_bridge::snapshot_from_world_with_camera(
                bridge.session.world(),
                Default::default(),
                &camera,
            ))
        }),
        true,
        move |key, state| input_runtime.borrow_mut().handle_key(key, state),
        |scene| {
            scene.set_lighting_mode(LightingMode::Lambert);
            scene.set_material(VisualAssets::SUN, MaterialInfo::sun());
            scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
        },
        GraphicsDebugConfig::default(),
    )
    .map(|report| {
        println!(
            "Visual finalizado: transiciones={} transición_gpu_ms={:?}",
            report.transitions, report.last_transition_ms,
        );
    })
    .map_err(|error| format!("ventana visual propia: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_keys_produce_distinct_individual_directions() {
        let cases = [
            (KeyCode::KeyW, GameplayVec3 { x: 0.0, y: 0.0, z: -1.0 }),
            (KeyCode::KeyS, GameplayVec3 { x: 0.0, y: 0.0, z: 1.0 }),
            (KeyCode::KeyA, GameplayVec3 { x: -1.0, y: 0.0, z: 0.0 }),
            (KeyCode::KeyD, GameplayVec3 { x: 1.0, y: 0.0, z: 0.0 }),
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
        assert_eq!(keys.input(), GameplayVec3 { x: 1.0, y: 0.0, z: -1.0 });

        keys.set(KeyCode::KeyW, false);
        keys.set(KeyCode::KeyA, true);
        assert_eq!(keys.input(), GameplayVec3 { x: 0.0, y: 0.0, z: 0.0 });

        keys.set(KeyCode::KeyD, false);
        keys.set(KeyCode::KeyS, true);
        assert_eq!(keys.input(), GameplayVec3 { x: -1.0, y: 0.0, z: 1.0 });
    }

    #[test]
    fn releasing_a_flight_key_preserves_other_active_inputs() {
        let mut keys = FlightKeys::default();
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyA, true);
        keys.set(KeyCode::KeyW, false);
        assert_eq!(keys.input(), GameplayVec3 { x: -1.0, y: 0.0, z: 0.0 });

        keys.set(KeyCode::KeyA, false);
        assert_eq!(keys.input(), GameplayVec3::ZERO);
    }

    #[test]
    fn simultaneous_flight_key_sequences_keep_only_remaining_contributions() {
        let mut keys = FlightKeys::default();
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyD, false);
        assert_eq!(keys.input(), GameplayVec3 { x: 0.0, y: 0.0, z: -1.0 });

        keys = FlightKeys::default();
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyS, true);
        keys.set(KeyCode::KeyD, false);
        assert_eq!(keys.input(), GameplayVec3 { x: 0.0, y: 0.0, z: 1.0 });

        keys = FlightKeys::default();
        keys.set(KeyCode::KeyW, true);
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyD, false);
        assert_eq!(keys.input(), GameplayVec3 { x: 0.0, y: 0.0, z: -1.0 });

        keys = FlightKeys::default();
        keys.set(KeyCode::KeyS, true);
        keys.set(KeyCode::KeyD, true);
        keys.set(KeyCode::KeyS, false);
        assert_eq!(keys.input(), GameplayVec3 { x: 1.0, y: 0.0, z: 0.0 });
    }

    #[test]
    fn repeated_press_does_not_accumulate_and_release_clears_each_key() {
        let mut keys = FlightKeys::default();
        for _ in 0..3 {
            keys.set(KeyCode::KeyD, true);
        }
        assert_eq!(keys.input(), GameplayVec3 { x: 1.0, y: 0.0, z: 0.0 });
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
        for _ in 0..12 {
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
        let before = bridge.camera(1.0).expect("camera should be valid").position();

        bridge.handle_key(KeyCode::ArrowLeft, ElementState::Pressed);

        let after = bridge.camera(1.0).expect("camera should remain valid").position();
        assert_ne!(after, before);
        assert!(after.is_finite());
    }
}