use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use engine::{Duration, EntityId, Vec3 as GameplayVec3};
use game::ProjectEchoSession;
use gameplay::types::ItemId;
use gameplay::{
    AttackType, BreakerId, BreakerState, Destination, DestinationId, DestinationType,
    DiscoveryState, ElectricalBreakerConfiguration, ElectricalPanelConfiguration,
    EnemyArchetype, EnemyConfiguration, EncounterId, EnvironmentalHazard, GameplayCommand, GameplayEvent,
    LifeSupportConfiguration, NavigationRoute, NavigationRouteState,
    OxygenCompartmentConfiguration, PlanetPhysics, SpatialEncounter, SpatialEncounterKind,
    InteractionBehavior, StationType, WeaponAmmunition, WeaponConfiguration,
};
use render_core::{LightingMode, MaterialInfo, VisualAssets};
use render_present::{
    run_hot_swap_window_with_input_and_scene_and_focus, ElementState, GraphicsDebugConfig,
    KeyCode, Preset, WindowConfig,
};

#[path = "gamepad.rs"]
mod gamepad;
#[path = "audio.rs"]
mod audio;
use gamepad::{ControllerButton, GamepadInput, HapticEvent};
use audio::{AudioEvent, AudioManager, SoundEffect};

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
            self.steps
                .push(format!("paso{}={command}:{:?}", self.evaluated - results.len() + index + 1, result.status));
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
}

struct VisualBridge {
    session: ProjectEchoSession,
    player: EntityId,
    ship: EntityId,
    pilot_station: EntityId,
    engineering_station: EntityId,
    electrical_panel: EntityId,
    life_support_panel: EntityId,
    bridge_door: EntityId,
    enemy: EntityId,
    feedback: String,
    observation: ObservationTransition,
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
    pilot_mode: bool,
    legacy_exterior_controls: bool,
    look_left: bool,
    look_right: bool,
    look_up: bool,
    look_down: bool,
    last_interior_update: Instant,
    last_world_update: Instant,
}

impl VisualBridge {
    #[cfg(test)]
    fn new() -> Result<Self, String> {
        Self::new_with_mode(false)
    }

    #[cfg(test)]
    fn new_with_mode(interior_mode: bool) -> Result<Self, String> {
        Self::new_with_mode_and_enemy_distance(interior_mode, false)
    }

    fn new_with_mode_and_enemy_distance(
        interior_mode: bool,
        safe_interior_enemy_start: bool,
    ) -> Result<Self, String> {
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
        let engineering_station = session
            .create_station(GameplayVec3::ZERO, 3.0, StationType::Engineering)
            .map_err(|error| format!("engineering station: {error:?}"))?;
        let _status_console = session
            .create_ship_status_console(
                GameplayVec3 {
                    x: -2.0,
                    y: 0.0,
                    z: 0.0,
                },
                3.0,
            )
            .map_err(|error| format!("crear consola de estado visual: {error:?}"))?;
        let electrical_panel = session
            .create_electrical_panel(
                GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -2.0,
                },
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
                GameplayVec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                },
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
                GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 2.0,
                },
                3.0,
            )
            .map_err(|error| format!("crear puerta del puente visual: {error:?}"))?;
        // Keep the initial interior encounter outside the player's immediate
        // weapon range. The enemy remains active and autonomous, but the
        // player gets a meaningful exploration window before combat begins.
        let enemy_position = if interior_mode && safe_interior_enemy_start {
            GameplayVec3 {
                x: 60.0,
                y: 0.0,
                z: -60.0,
            }
        } else {
            GameplayVec3 {
                x: 18.0,
                y: 0.0,
                z: -18.0,
            }
        };
        let enemy_configuration = EnemyConfiguration::for_archetype(
            EnemyArchetype::Shooter,
            enemy_position,
        );
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
            engineering_station,
            electrical_panel,
            life_support_panel,
            bridge_door,
            enemy,
            feedback: "ESCENA INICIALIZADA".to_owned(),
            observation: ObservationTransition::default(),
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
            pilot_mode: false,
            legacy_exterior_controls: !interior_mode,
            look_left: false,
            look_right: false,
            look_up: false,
            look_down: false,
            last_interior_update: Instant::now(),
            last_world_update: Instant::now(),
        })
    }

    fn new_visual_runtime() -> Result<Self, String> {
        let mut bridge = Self::new_with_mode_and_enemy_distance(true, true)?;
        bridge.started = false;
        bridge.feedback = "ASTRAL ECHO - PULSA ENTER PARA COMENZAR".to_owned();
        Ok(bridge)
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
        self.session.update([GameplayCommand::AdvanceTime { delta }]);
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
        self.refresh_player_defeat_state();
        let accepted = results.iter().all(|result| {
            matches!(result.status, gameplay::CommandStatus::Success { .. })
        });
        let events = self
            .session
            .world()
            .events()
            .iter()
            .map(|value| format!("{:?}", value));
        aggregate.record(&commands, &results, events);
        accepted
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
        });
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
        let player = self.player;
        let ship = self.ship;
        let enemy = self.enemy;
        let pilot_station = self.pilot_station;
        if self.player_defeated {
            if state == ElementState::Pressed {
                match key {
                    KeyCode::KeyO => self.toggle_observation(),
                    KeyCode::ArrowLeft => {
                        self.observation.adjust_orbit(-OBSERVATION_AZIMUTH_STEP, 0.0)
                    }
                    KeyCode::ArrowRight => {
                        self.observation.adjust_orbit(OBSERVATION_AZIMUTH_STEP, 0.0)
                    }
                    KeyCode::ArrowUp => {
                        self.observation.adjust_orbit(0.0, OBSERVATION_ELEVATION_STEP)
                    }
                    KeyCode::ArrowDown => {
                        self.observation.adjust_orbit(0.0, -OBSERVATION_ELEVATION_STEP)
                    }
                    _ => {}
                }
            }
            return;
        }
        if self.flight_keys.set(key, state == ElementState::Pressed) {
            if self.pilot_mode || self.legacy_exterior_controls {
                self.accelerate_ship(player, pilot_station, self.current_flight_input());
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
            KeyCode::KeyI => self.guided_interaction(),
            KeyCode::KeyR => self.restore_energy(),
            KeyCode::KeyY => self.restore_life_support(),
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
            KeyCode::KeyC => self.equip_kinetic_weapon(),
            KeyCode::KeyF => {
                let mission_five_active = self
                    .session
                    .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                    .is_some_and(|mission| {
                        mission.state == gameplay::domains::mission::MissionState::Active
                    });
                if !mission_five_active && self.session.world().landing_available(DESTINATION) {
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
            KeyCode::KeyP if self.pilot_mode || !self.interior_mode => {
                self.toggle_flight(player, pilot_station)
            }
            KeyCode::KeyO => self.toggle_observation(),
            KeyCode::ArrowLeft if self.interior_mode || self.pilot_mode => {
                self.look_left = true;
                if self.interior_mode { self.rotate_player(-6.0, 0.0); }
                if self.pilot_mode { self.observation.adjust_orbit(-OBSERVATION_AZIMUTH_STEP, 0.0); }
            }
            KeyCode::ArrowRight if self.interior_mode || self.pilot_mode => {
                self.look_right = true;
                if self.interior_mode { self.rotate_player(6.0, 0.0); }
                if self.pilot_mode { self.observation.adjust_orbit(OBSERVATION_AZIMUTH_STEP, 0.0); }
            }
            KeyCode::ArrowUp if self.interior_mode || self.pilot_mode => {
                self.look_up = true;
                if self.interior_mode { self.rotate_player(0.0, -4.0); }
                if self.pilot_mode { self.observation.adjust_orbit(0.0, OBSERVATION_ELEVATION_STEP); }
            }
            KeyCode::ArrowDown if self.interior_mode || self.pilot_mode => {
                self.look_down = true;
                if self.interior_mode { self.rotate_player(0.0, 4.0); }
                if self.pilot_mode { self.observation.adjust_orbit(0.0, -OBSERVATION_ELEVATION_STEP); }
            }
            KeyCode::ArrowLeft => self.observation.adjust_orbit(-OBSERVATION_AZIMUTH_STEP, 0.0),
            KeyCode::ArrowRight => self.observation.adjust_orbit(OBSERVATION_AZIMUTH_STEP, 0.0),
            KeyCode::ArrowUp => self.observation.adjust_orbit(0.0, OBSERVATION_ELEVATION_STEP),
            KeyCode::ArrowDown => self.observation.adjust_orbit(0.0, -OBSERVATION_ELEVATION_STEP),
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
        if self.session.mission_progress(mission).is_some_and(|value| {
            value.state == gameplay::domains::mission::MissionState::Available
        }) {
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
            let started = self.issue_into([GameplayCommand::StartMission { mission }], &mut aggregate);
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
        } else if self.session.mission_progress(mission).is_some_and(|value| {
            value.state == gameplay::domains::mission::MissionState::Active
        }) {
            self.feedback = "MISION YA ACTIVA".to_owned();
            println!("visual gameplay: {}", self.feedback);
            return;
        } else if self.session.mission_progress(mission).is_some_and(|value| {
            value.state == gameplay::domains::mission::MissionState::Completed
        }) {
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
            DiscoveryState::Known => {
                !matches!(existing.discovery, DiscoveryState::Unknown)
            }
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
        self.session
            .world()
            .enemies()
            .any(|value| {
                value.active
                    && value.archetype == Some(EnemyArchetype::Shooter)
                    && !value.is_defeated()
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
            self.session.world().ship_position().unwrap_or(GameplayVec3::ZERO),
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
        let position = self
            .session
            .world()
            .player(self.player)
            .map(|player| player.position)
            .unwrap_or(GameplayVec3::ZERO);
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
        let can_access_pilot_console = !self.pilot_mode
            && !self.legacy_exterior_controls
            && self
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .is_some_and(|mission| {
                    mission.state == gameplay::domains::mission::MissionState::Completed
                })
            && self
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .is_some_and(|mission| {
                    mission.state == gameplay::domains::mission::MissionState::Available
                });
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
        let action = self
            .session
            .world()
            .guidance_for_player(self.player)
            .ok()
            .and_then(|guidance| guidance.recommended_action);
        let pilot_console_available = action.is_none()
            && !self.pilot_mode
            && !self.legacy_exterior_controls
            && self
                .session
                .mission_progress(crate::el_despertar::EL_DESPERTAR)
                .is_some_and(|mission| {
                    mission.state == gameplay::domains::mission::MissionState::Completed
                })
            && self
                .session
                .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .is_some_and(|mission| {
                    mission.state == gameplay::domains::mission::MissionState::Available
                });
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
                && self
                    .session
                    .mission_progress(crate::el_despertar::EL_DESPERTAR)
                    .is_some_and(|mission| {
                        mission.state == gameplay::domains::mission::MissionState::Completed
                    })
                && self
                    .session
                    .mission_progress(crate::ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                    .is_some_and(|mission| {
                        mission.state == gameplay::domains::mission::MissionState::Available
                    });
            let behavior = self
                .session
                .world()
                .interactable(target)
                .map(|value| value.behavior);
            if target == self.bridge_door {
                self.issue_into([GameplayCommand::UnlockDoor {
                    door: self.bridge_door,
                }], &mut aggregate);
            }
            if target == self.pilot_station {
                self.issue_into([GameplayCommand::ReleaseStation {
                    station: self.pilot_station,
                    player: self.player,
                }], &mut aggregate);
            }
            let interaction_accepted = self.issue_into([GameplayCommand::Interact {
                player: self.player,
                target,
            }], &mut aggregate);

            // Panel interactions authorize the corresponding gameplay systems;
            // the follow-up commands still perform all normal range,
            // configuration and dependency validation in gameplay.
            if interaction_accepted {
                match behavior {
                    Some(InteractionBehavior::ElectricalPanel) => {
                        self.issue_into([GameplayCommand::SetElectricalBreakerState {
                            player: self.player,
                            panel: target,
                            breaker: BreakerId::new(1),
                            state: BreakerState::On,
                        }], &mut aggregate);
                    }
                    Some(InteractionBehavior::LifeSupportPanel) => {
                        self.issue_into([GameplayCommand::SetOxygenAllocation {
                            player: self.player,
                            panel: target,
                            compartment: gameplay::CompartmentId::new(1),
                            allocation: 3,
                        }], &mut aggregate);
                        self.issue_into([GameplayCommand::SetOxygenAllocation {
                            player: self.player,
                            panel: target,
                            compartment: gameplay::CompartmentId::new(2),
                            allocation: 2,
                        }], &mut aggregate);
                    }
                    _ => {}
                }
            }
            let completed = self.finish_issue(aggregate);
            if completed && pilot_access_requested {
                self.enter_pilot();
                return;
            }
            if completed && target == self.bridge_door && self.interior_mode {
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
                if completed && target == self.bridge_door && self.interior_mode {
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

    fn update_interior_motion(&mut self) {
        if !self.interior_mode || self.player_defeated {
            return;
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
        let collision_buffer = 0.75;
        let min_x = -game::render_bridge::ECHO7_INTERIOR_HALF_WIDTH
            + game::render_bridge::ECHO7_PLAYER_RADIUS
            + collision_buffer;
        let max_x = game::render_bridge::ECHO7_INTERIOR_HALF_WIDTH
            - game::render_bridge::ECHO7_PLAYER_RADIUS
            - collision_buffer;
        let min_z = -game::render_bridge::ECHO7_INTERIOR_HALF_DEPTH
            + game::render_bridge::ECHO7_PLAYER_RADIUS
            + collision_buffer;
        let max_z = game::render_bridge::ECHO7_INTERIOR_HALF_DEPTH
            - game::render_bridge::ECHO7_PLAYER_RADIUS
            - collision_buffer;
        let braking_distance_x = player.velocity.x.abs().powi(2)
            / (2.0 * player.movement.deceleration.max(1.0));
        let braking_distance_z = player.velocity.z.abs().powi(2)
            / (2.0 * player.movement.deceleration.max(1.0));
        if player.velocity.x > 0.0
            && max_x - player.position.x <= braking_distance_x + player.movement.max_speed * seconds
        {
            direction.x = -1.0;
        } else if player.velocity.x < 0.0
            && player.position.x - min_x <= braking_distance_x + player.movement.max_speed * seconds
        {
            direction.x = 1.0;
        } else if (player.position.x <= min_x && direction.x < 0.0)
            || (player.position.x >= max_x && direction.x > 0.0)
        {
            direction.x = 0.0;
        }
        if player.velocity.z > 0.0
            && max_z - player.position.z <= braking_distance_z + player.movement.max_speed * seconds
        {
            direction.z = -1.0;
        } else if player.velocity.z < 0.0
            && player.position.z - min_z <= braking_distance_z + player.movement.max_speed * seconds
        {
            direction.z = 1.0;
        } else if (player.position.z <= min_z && direction.z < 0.0)
            || (player.position.z >= max_z && direction.z > 0.0)
        {
            direction.z = 0.0;
        }
        let magnitude = (direction.x * direction.x + direction.z * direction.z).sqrt();
        if magnitude > 1.0 {
            direction.x /= magnitude;
            direction.z /= magnitude;
        }
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
    }

    fn contextual_prompt(&self) -> Option<String> {
        self.selected_interactable().map(|interactable| {
            let label = match interactable.behavior {
                InteractionBehavior::Door { .. } => "PUERTA DEL PUENTE [I] ABRIR",
                InteractionBehavior::Station { station } => match station.station_type {
                    StationType::Piloting => "CONSOLA DE PILOTAJE [I] ACCEDER",
                    StationType::Navigation => "ESTACION DE NAVEGACION [I] ACCEDER",
                    StationType::Engineering => "ESTACION DE INGENIERIA [I] ACCEDER",
                    StationType::LifeSupport => "ESTACION DE SOPORTE VITAL [I] ACCEDER",
                    StationType::Weapons => "ESTACION DE ARMAS [I] ACCEDER",
                },
                InteractionBehavior::ElectricalPanel => "PANEL ELECTRICO [I] REPARAR",
                InteractionBehavior::LifeSupportPanel => "SOPORTE VITAL [I] REPARAR",
                InteractionBehavior::Console { role, .. } => match role {
                    gameplay::ConsoleRole::ShipStatus => "CONSOLA DE ESTADO [I] CONSULTAR",
                    gameplay::ConsoleRole::Generic => "CONSOLA [I] INTERACTUAR",
                },
            };
            label.to_owned()
        })
    }

    fn selected_interactable(&self) -> Option<&gameplay::Interactable> {
        if self.pilot_mode {
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
            return "PILOTAJE ACTIVO: WASD VOLAR - FLECHAS RUMBO - P AUTOPILOTO - I SALIR".to_owned();
        }
        if let Some(prompt) = self.contextual_prompt() {
            return format!("{prompt} - ACERCATE Y USA I");
        }
        if self.interior_mode {
            if self
                .guidance_prompt()
                .is_some_and(|value| value.starts_with('M'))
            {
                return "PULSA M PARA INICIAR LA MISION DISPONIBLE".to_owned();
            }
            return "EXPLORACION: WASD MOVER - FLECHAS MIRAR - I INTERACTUAR".to_owned();
        }
        "EXTERIOR: SIGUE LA GUIA Y USA Q, P, F O E SEGUN EL OBJETIVO".to_owned()
    }

    fn guidance_prompt(&self) -> Option<String> {
        let guidance = self.session.world().guidance_for_player(self.player).ok()?;
        let objective = guidance.current_objective.and_then(|id| {
            guidance.objectives.iter().find(|value| value.id == id)
        });
        let Some(objective) = objective else {
            return self
                .session
                .world()
                .missions()
                .find(|mission| mission.state() == gameplay::domains::mission::MissionState::Available)
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
                "{} | PENDIENTE: {}",
                guidance_action_label(objective.recommended_action),
                guidance_requirement_label(*requirement),
            ));
        }
        Some(guidance_action_label(objective.recommended_action))
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
            format!("MODO PILOTAJE ECHO-7 | WASD VOLAR | FLECHAS RUMBO | P AUTOPILOTO | I SALIR | AUTO: {autopilot}")
        } else if self.interior_mode {
            "WASD MOVER | FLECHAS MIRAR | I USAR | M INICIAR".to_owned()
        } else if self
            .session
            .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
            .is_some_and(|mission| mission.state == gameplay::domains::mission::MissionState::Active)
        {
            "Q ESCANEAR | P VIAJAR | F ATACAR | E RECOGER | T RECUPERAR".to_owned()
        } else {
            "Q ESCANEAR | P VIAJAR | F ATACAR | E RECOGER".to_owned()
        }
    }

    fn restore_energy(&mut self) {
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into([GameplayCommand::Interact {
            player: self.player,
            target: self.electrical_panel,
        }], &mut aggregate);
        self.issue_into([GameplayCommand::SetElectricalBreakerState {
            player: self.player,
            panel: self.electrical_panel,
            breaker: BreakerId::new(1),
            state: BreakerState::On,
        }], &mut aggregate);
        self.finish_issue(aggregate);
    }

    fn restore_life_support(&mut self) {
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into([GameplayCommand::Interact {
            player: self.player,
            target: self.life_support_panel,
        }], &mut aggregate);
        self.issue_into([GameplayCommand::SetOxygenAllocation {
            player: self.player,
            panel: self.life_support_panel,
            compartment: gameplay::CompartmentId::new(1),
            allocation: 3,
        }], &mut aggregate);
        self.issue_into([GameplayCommand::SetOxygenAllocation {
            player: self.player,
            panel: self.life_support_panel,
            compartment: gameplay::CompartmentId::new(2),
            allocation: 2,
        }], &mut aggregate);
        self.finish_issue(aggregate);
    }

    fn accelerate_ship(&mut self, player: EntityId, pilot_station: EntityId, input: GameplayVec3) {
        let mut aggregate = FeedbackAggregate::default();
        self.issue_into([
            GameplayCommand::AccelerateShip {
                player,
                pilot_station,
                input,
            },
            GameplayCommand::AdvanceTime {
                delta: Duration(100),
            },
        ], &mut aggregate);
        self.finish_issue(aggregate);
    }

    fn enter_pilot(&mut self) {
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
            self.pilot_mode = true;
            self.interior_mode = false;
            self.feedback = "PILOTAJE DE ECHO-7 ACTIVO - I SALIR, P AUTOPILOTO".to_owned();
        } else {
            self.feedback = aggregate.message();
        }
    }

    fn exit_pilot(&mut self) {
        let mut aggregate = FeedbackAggregate::default();
        let accepted = self.issue_into(
            [GameplayCommand::SetShipControlActive {
                player: self.player,
                pilot_station: self.pilot_station,
                active: false,
            }],
            &mut aggregate,
        );
        if accepted {
            self.issue_into(
                [GameplayCommand::ReleaseStation {
                    station: self.pilot_station,
                    player: self.player,
                }],
                &mut aggregate,
            );
            self.pilot_mode = false;
            self.interior_mode = true;
            self.flight_keys = FlightKeys::default();
            self.gamepad_flight = GameplayVec3::ZERO;
            self.feedback = "PILOTAJE FINALIZADO - CONTROL DE PERSONAJE RESTAURADO".to_owned();
        } else {
            self.feedback = aggregate.message();
        }
    }

    fn update_pilot_motion(&mut self) {
        if !self.pilot_mode || self.player_defeated {
            return;
        }
        let yaw = self.look_right as i8 - self.look_left as i8;
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
    }

    fn current_flight_input(&self) -> GameplayVec3 {
        let keyboard = self.flight_keys.input();
        GameplayVec3 {
            x: (keyboard.x + self.gamepad_flight.x).clamp(-1.0, 1.0),
            y: 0.0,
            z: (keyboard.z + self.gamepad_flight.z).clamp(-1.0, 1.0),
        }
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
        if !self.pilot_mode && !self.legacy_exterior_controls {
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
                gameplay::GameplayVisualEventKind::Attack
                    if event.source == Some(self.player) => Some(HapticEvent::AttackExecuted),
                gameplay::GameplayVisualEventKind::EnemyDefeated => {
                    Some(HapticEvent::EnemyDefeated)
                }
                gameplay::GameplayVisualEventKind::ItemCollected
                    if event.source == Some(self.player) => Some(HapticEvent::ItemCollected),
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
                gameplay::GameplayVisualEventKind::Attack
                    if event.source == Some(self.player) => Some(SoundEffect::PlayerAttack),
                gameplay::GameplayVisualEventKind::EnemyDefeated => {
                    Some(SoundEffect::EnemyDefeated)
                }
                gameplay::GameplayVisualEventKind::ItemCollected
                    if event.source == Some(self.player) => Some(SoundEffect::Collect),
                _ => None,
            };
            if let Some(effect) = effect {
                self.audio.dispatch_event(AudioEvent { id: event.id, effect });
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
        let now = self.session.world().time().elapsed.0;
        self.observation.toggle(now);
    }

    fn toggle_flight(&mut self, player: EntityId, pilot_station: EntityId) {
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
        self.issue_into([
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
        ], &mut aggregate);
        self.finish_issue(aggregate);
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
        if self.interior_mode {
            return game::render_bridge::interior_camera_for_player(world, self.player, aspect);
        }
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
    let runtime = Rc::new(RefCell::new(VisualBridge::new_visual_runtime()?));
    let snapshot_runtime = Rc::clone(&runtime);
    let input_runtime = Rc::clone(&runtime);
    let focus_runtime = Rc::clone(&runtime);
    let config = WindowConfig {
        title: "Astral Echo — CPU propio".into(),
        preset: Preset::Low,
        raster_controls: false,
        ..Default::default()
    };
    run_hot_swap_window_with_input_and_scene_and_focus(
        config,
        Box::new(move |width, height| {
            let mut bridge = snapshot_runtime.borrow_mut();
            bridge.advance_visual_time();
            bridge.update_interior_motion();
            bridge.update_pilot_motion();
            bridge.poll_gamepad();
            let camera = bridge
                .camera(width as f32 / height.max(1) as f32)
                .map_err(|error| format!("cámara visual: {error:?}"))?;
            let mut snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view(
                bridge.session.world(),
                Default::default(),
                &camera,
                bridge.interior_mode,
            );
            if snapshot.overlay.game_over {
                snapshot.overlay.hud.interaction_prompt = None;
                snapshot.overlay.hud.guidance = None;
                snapshot.overlay.hud.controls = None;
                snapshot.overlay.hud.instructions = None;
                snapshot.overlay.hud.feedback = Some("JUGADOR DERROTADO".to_owned());
            } else {
                snapshot.overlay.hud.interaction_prompt = bridge.contextual_prompt();
                snapshot.overlay.hud.guidance = bridge.guidance_prompt();
                snapshot.overlay.hud.controls = Some(bridge.controls_hint());
                snapshot.overlay.hud.instructions = Some(bridge.instructions_hint());
                snapshot.overlay.hud.feedback = Some(bridge.feedback.clone());
            }
            Ok(snapshot)
        }),
        true,
        move |key, state| input_runtime.borrow_mut().handle_key(key, state),
        move |focused| {
            let mut bridge = focus_runtime.borrow_mut();
            bridge.audio.on_focus(focused);
            if !focused {
                bridge.neutralize_gamepad();
            }
        },
        |scene| {
            scene.set_lighting_mode(LightingMode::Lambert);
            let assets_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models");
            for (path, mesh, material) in [
                (assets_root.join("station/station.obj"), render_core::MeshHandle::STATION, VisualAssets::STATION),
                (assets_root.join("shooter/shooter.obj"), render_core::MeshHandle::ENEMY_PLACEHOLDER, VisualAssets::ENEMY),
            ] {
                match render_core::load_obj(&path) {
                    Ok(mesh_data) => scene.register_mesh(mesh, material, mesh_data),
                    Err(error) => eprintln!("modelo no cargado {}: {error}", path.display()),
                }
            }
            scene.set_material(VisualAssets::SUN, MaterialInfo::sun());
            scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
            scene.set_material(VisualAssets::STATION, MaterialInfo::station());
            scene.set_material(VisualAssets::DERELICT, MaterialInfo::derelict());
            scene.set_material(VisualAssets::ASTEROID, MaterialInfo::asteroid());
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
        Some(gameplay::GuidanceAction::CompleteObjective(_)) => "SIGUE LOS REQUISITOS DEL OBJETIVO".to_owned(),
        None => "SIN ACCION RECOMENDADA".to_owned(),
    }
}

fn guidance_requirement_label(requirement: gameplay::GuidanceRequirement) -> String {
    match requirement {
        gameplay::GuidanceRequirement::MissionActive(_) => "INICIA LA MISION CON M".to_owned(),
        gameplay::GuidanceRequirement::MissionCompleted(_) => "COMPLETA LA MISION PREVIA".to_owned(),
        gameplay::GuidanceRequirement::ObjectiveCompleted(_) => "COMPLETA EL OBJETIVO PREVIO".to_owned(),
        gameplay::GuidanceRequirement::KnowledgeAtLeast { .. } => "USA Q PARA ESCANEAR".to_owned(),
        gameplay::GuidanceRequirement::DestinationDiscovered(_) => "DESCUBRE EL DESTINO CON P".to_owned(),
        gameplay::GuidanceRequirement::DestinationArrived(_) => "LLEGA AL DESTINO CON P".to_owned(),
        gameplay::GuidanceRequirement::EnemyDefeated(_) => "DERROTA LA AMENAZA CON F".to_owned(),
        gameplay::GuidanceRequirement::ResourceObtained { .. } => "RECOGE EL RECURSO CON E".to_owned(),
        gameplay::GuidanceRequirement::WorldActivityCompleted(_) => "COMPLETA LA ACTIVIDAD DEL MUNDO".to_owned(),
        gameplay::GuidanceRequirement::DerelictResolved(_) => "RESUELVE LA ESTACION ABANDONADA".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_runtime_requires_explicit_start_and_preserves_initial_campaign_state() {
        let mut bridge = VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        assert!(!bridge.started);
        assert!(bridge.feedback.contains("ASTRAL ECHO"));
        assert_eq!(bridge.session.world().time().elapsed.0, 0);

        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(bridge.session.world().time().elapsed.0, 0);

        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        assert!(bridge.started);
        assert!(bridge.feedback.contains("ECHO-7"));
        assert_eq!(bridge.session.world().time().elapsed.0, 0);
        assert_eq!(bridge.session.mission_progress(game::campaign::CAMPAIGN_ORDER[0]).unwrap().state,
            gameplay::domains::mission::MissionState::Locked);
    }

    #[test]
    fn visual_runtime_advances_gameplay_time_while_active() {
        let mut bridge = VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(bridge.session.world().time().elapsed.0, 100);
        assert_eq!(bridge.session.world().time().tick.0, 1);
    }

    #[test]
    fn visual_runtime_pauses_time_after_player_defeat() {
        let mut bridge = VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
        bridge.handle_key(KeyCode::Enter, ElementState::Pressed);
        bridge.advance_visual_time_by(Duration(100));
        bridge.player_defeated = true;
        bridge.advance_visual_time_by(Duration(100));
        assert_eq!(bridge.session.world().time().elapsed.0, 100);
        assert_eq!(bridge.session.world().time().tick.0, 1);
    }

    #[test]
    fn initial_playable_cycle_keeps_one_session_through_pilot_and_return() {
        let mut bridge = VisualBridge::new_visual_runtime().expect("visual runtime should initialize");
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

        // The first five interactions complete Mission1's real interior
        // objectives. The next interaction explicitly enters the nearby
        // piloting console instead of changing mode as a side effect.
        for _ in 0..6 {
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
        assert_eq!(session_address, (&bridge.session as *const ProjectEchoSession) as usize);
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
        assert_eq!(bridge.session.world().ship_position(), Some(GameplayVec3::ZERO));
    }

    #[test]
    fn pilot_mode_is_entered_from_the_pilot_console_and_exits_to_character_control() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.guided_interaction();
        assert!(bridge.pilot_mode, "the nearby piloting console should enter pilot mode");
        assert!(!bridge.interior_mode);
        assert_eq!(bridge.session.world().ship_control_active(), Some(true));

        bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        assert!(!bridge.pilot_mode);
        assert!(bridge.interior_mode);
        assert_eq!(bridge.session.world().ship_control_active(), Some(false));
        let ship_after_exit = bridge.session.world().ship_position().unwrap();
        let heading_after_exit = bridge.session.world().ship_orientation().unwrap();
        bridge.guided_interaction();
        assert!(bridge.pilot_mode);
        assert_eq!(bridge.session.world().ship_position(), Some(ship_after_exit));
        assert_eq!(bridge.session.world().ship_orientation(), Some(heading_after_exit));
    }

    #[test]
    fn pilot_mode_moves_and_rotates_the_authoritative_ship_only() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        bridge.guided_interaction();
        assert!(bridge.pilot_mode);
        let player_before = bridge.session.world().player(bridge.player).unwrap().position;
        let ship_before = bridge.session.world().ship_position().unwrap();
        let orientation_before = bridge.session.world().ship_orientation().unwrap();

        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.handle_key(KeyCode::ArrowRight, ElementState::Pressed);
        bridge.update_pilot_motion();

        assert_ne!(bridge.session.world().ship_position().unwrap(), ship_before);
        assert_ne!(bridge.session.world().ship_orientation().unwrap(), orientation_before);
        assert_eq!(bridge.session.world().player(bridge.player).unwrap().position, player_before);
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
        assert!(bridge.guidance_prompt().is_some_and(|value| value.contains("M")));
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
        assert!(bridge.instructions_hint().contains("USA I"));
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
        assert!(position.x < 8.4, "player crossed the interior clearance: {position:?}");
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
        assert!(position.x.abs() <= 7.8, "player escaped ECHO-7 width: {position:?}");
        assert!(position.z.abs() <= 5.8, "player escaped ECHO-7 depth: {position:?}");
    }

    #[test]
    fn character_camera_follows_the_validated_player_position() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        let before = bridge.camera(16.0 / 9.0).expect("character camera should be valid").position();
        bridge.handle_key(KeyCode::KeyW, ElementState::Pressed);
        bridge.update_interior_motion();
        let after = bridge.camera(16.0 / 9.0).expect("character camera should remain valid").position();
        assert_ne!(before, after);
        assert!(after.y > -1.2 && after.y < 6.4);
    }

    #[test]
    fn echo7_uses_ship_representation_and_not_station_model() {
        let bridge = VisualBridge::new_with_mode(true).expect("visual bootstrap should work");
        let camera = bridge.camera(16.0 / 9.0).expect("character camera should be valid");
        let snapshot = game::render_bridge::snapshot_from_world_with_camera_and_view(
            bridge.session.world(),
            Default::default(),
            &camera,
            true,
        );
        let echo7 = snapshot.nodes.iter().find(|node| node.entity == render_core::SnapshotId::Entity(bridge.ship));
        assert_eq!(echo7.map(|node| node.mesh), Some(game::render_bridge::ECHO7_MESH));
        assert_ne!(game::render_bridge::ECHO7_MESH, game::render_bridge::STATION_MESH);
        assert!(snapshot.nodes.iter().filter(|node| node.mesh == game::render_bridge::STATION_MESH).count() >= 2);
    }

    #[test]
    fn controller_bindings_reuse_existing_keyboard_actions() {
        assert_eq!(controller_key(ControllerButton::South), Some(KeyCode::KeyF));
        assert_eq!(controller_key(ControllerButton::RightTrigger), Some(KeyCode::KeyF));
        assert_eq!(controller_key(ControllerButton::West), Some(KeyCode::KeyQ));
        assert_eq!(controller_key(ControllerButton::North), Some(KeyCode::KeyO));
        assert_eq!(controller_key(ControllerButton::DPadLeft), Some(KeyCode::ArrowLeft));
    }

    #[test]
    fn feedback_aggregate_classifies_all_result_combinations() {
        let success = gameplay::CommandResult::success(true);
        let failure = gameplay::CommandResult::rejected(gameplay::CommandRejection::InvalidDuration);

        let cases = [
            (&[success, success][..], FeedbackOutcome::Complete, true, "OPERACION COMPLETADA"),
            (&[failure, failure][..], FeedbackOutcome::Failed, false, "OPERACION FALLIDA"),
            (&[success, failure][..], FeedbackOutcome::Partial, false, "PARCIALMENTE COMPLETADA"),
            (&[][..], FeedbackOutcome::Empty, false, "SIN RESULTADO EVALUABLE"),
        ];
        for (results, outcome, complete, label) in cases {
            let mut aggregate = FeedbackAggregate::default();
            let commands = vec![GameplayCommand::AdvanceTime {
                delta: Duration(100),
            }; results.len()];
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
        bridge.set_gamepad_flight(GameplayVec3 { x: 1.0, y: 0.0, z: -1.0 });
        bridge.poll_gamepad();

        assert_eq!(bridge.session.world().time(), defeated_time);
        assert_eq!(bridge.gamepad_flight, GameplayVec3::ZERO);
        assert_eq!(bridge.feedback, defeated_feedback);
        let snapshot = game::render_bridge::snapshot_from_world(
            bridge.session.world(),
            Default::default(),
        );
        assert!(snapshot.overlay.game_over);
        assert_eq!(snapshot.overlay.hud.feedback.as_deref(), Some("JUGADOR DERROTADO"));
        assert!(snapshot.nodes.iter().all(|node| {
            node.transform.translation.is_finite()
                && node.transform.rotation.is_finite()
                && node.transform.scale.is_finite()
        }));
    }

    #[test]
    fn interior_start_keeps_enemy_outside_immediate_weapon_range() {
        let mut bridge = VisualBridge::new_with_mode_and_enemy_distance(true, true)
            .expect("visual bridge should initialize");
        let enemy = bridge
            .session
            .world()
            .combatant(bridge.enemy)
            .copied()
            .expect("visual enemy should exist");
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
        assert!(!bridge.session.world().events().iter().any(|event| {
            matches!(event, GameplayEvent::CombatDamageApplied { target, .. } if *target == bridge.player)
        }));
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
        assert_eq!(bridge.session.world().combatants().filter(|value| {
            !value.is_player
                && value.active
                && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
        }).count(), 1);
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

        let snapshot = game::render_bridge::snapshot_from_world(
            bridge.session.world(),
            Default::default(),
        );
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
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
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
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        for _ in 0..5 {
            bridge.handle_key(KeyCode::KeyI, ElementState::Pressed);
        }
        bridge
            .session
            .register_destination(Destination {
                id: crate::ecos_anomalia::PLANET_DESTINATION,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3 { x: 99.0, y: 0.0, z: 99.0 },
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
            bridge.session.world().destination(crate::ecos_anomalia::PLANET_DESTINATION),
            Some(&Destination {
                id: crate::ecos_anomalia::PLANET_DESTINATION,
                destination_type: DestinationType::SpacePoint,
                position: GameplayVec3 { x: 99.0, y: 0.0, z: 99.0 },
                travel_duration: Duration(1000),
                fuel_cost: 1,
                discovery: DiscoveryState::Known,
            })
        );
        assert!(bridge.feedback.contains("DESTINO INCOMPATIBLE"));
    }

    #[test]
    fn mission_two_completes_all_objectives_through_real_gameplay_systems() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);

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
        assert!(matches!(
            collected.first().map(|value| &value.status),
            Some(gameplay::CommandStatus::Success { changed: true })
        ), "collect result: {:?}, player: {:?}, reward: {:?}", collected, bridge.session.world().player(bridge.player), bridge.session.world().collectible(reward.entity));
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
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);

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
        assert_eq!(mission_two.state, gameplay::domains::mission::MissionState::Completed);
        let mission_three = bridge
            .session
            .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
            .expect("Mission3 should exist");
        assert_eq!(mission_three.state, gameplay::domains::mission::MissionState::Available);

        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let mission_three = bridge
            .session
            .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
            .expect("Mission3 should exist after start");
        assert_eq!(mission_three.state, gameplay::domains::mission::MissionState::Active);
        let destination_count = bridge.session.world().destinations().count();
        let hazard_count = bridge.session.world().environmental_hazards().count();
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        assert_eq!(bridge.session.world().destinations().count(), destination_count);
        assert_eq!(bridge.session.world().environmental_hazards().count(), hazard_count);
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
        assert_eq!(
            bridge
                .session
                .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
                .expect("Mission3 progress should exist")
                .state,
            gameplay::domains::mission::MissionState::Completed,
            "progress={:?}, travel={:?}, position={:?}, events={:?}",
            bridge.session.mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL),
            bridge.session.world().travel_state(),
            bridge.session.world().ship_position(),
            bridge.session.world().events()
        );
    }

    #[test]
    fn campaign_missions_one_through_five_complete_end_to_end() {
        let mut bridge = VisualBridge::new_with_mode(true).expect("visual bridge should initialize");
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
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
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);
        bridge.session.update([GameplayCommand::AdvanceTime {
            delta: Duration(1),
        }]);

        let mission_three = bridge
            .session
            .mission_progress(crate::tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
            .expect("Mission3 should exist");
        assert_eq!(mission_three.state, gameplay::domains::mission::MissionState::Completed);
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
        assert_eq!(mission_five.state, gameplay::domains::mission::MissionState::Available);
        bridge.handle_key(KeyCode::KeyM, ElementState::Pressed);
        let mission_five = bridge
            .session
            .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
            .expect("Mission5 should exist after start");
        assert_eq!(mission_five.state, gameplay::domains::mission::MissionState::Active);
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
}
