use std::cell::RefCell;
use std::rc::Rc;

use engine::{Duration, EntityId, Vec3 as GameplayVec3};
use game::ProjectEchoSession;
use gameplay::types::ItemId;
use gameplay::{
    AttackType, BreakerId, BreakerState, Destination, DestinationId, DestinationType,
    DiscoveryState, ElectricalBreakerConfiguration, ElectricalPanelConfiguration,
    EnemyArchetype, EnemyConfiguration, EncounterId, GameplayCommand, GameplayEvent,
    LifeSupportConfiguration, NavigationRoute, NavigationRouteState,
    OxygenCompartmentConfiguration, PlanetPhysics, SpatialEncounter, SpatialEncounterKind,
    StationType, WeaponAmmunition, WeaponConfiguration,
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
        let mut audio = AudioManager::new();
        audio.request_exploration_music();
        Ok(Self {
            session,
            player,
            ship,
            pilot_station,
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
        })
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
    ) {
        if self.player_defeated {
            return;
        }
        let commands: Vec<_> = commands.into_iter().collect();
        let results = self.session.update(commands.clone());
        self.refresh_player_defeat_state();
        let events = self
            .session
            .world()
            .events()
            .iter()
            .map(|value| format!("{:?}", value));
        aggregate.record(&commands, &results, events);
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
            self.accelerate_ship(player, pilot_station, self.current_flight_input());
            return;
        }
        if state != ElementState::Pressed {
            return;
        }
        self.last_discrete_action = Some(key);
        match key {
            KeyCode::KeyM => self.start_presented_mission(),
            KeyCode::KeyT => self.retry_cooperative_mission_content(),
            KeyCode::KeyI => self.guided_interaction(),
            KeyCode::KeyR => self.restore_energy(),
            KeyCode::KeyY => self.restore_life_support(),
            KeyCode::Space => {
                self.issue([GameplayCommand::AdvanceTime {
                    delta: Duration(100),
                }]);
            }
            KeyCode::KeyQ => {
                if self.issue([GameplayCommand::ScanSignals { player, ship }]) {
                    self.audio.request_effect(SoundEffect::Scan);
                }
            }
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
            self.issue_into([GameplayCommand::StartMission { mission }], &mut aggregate);
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
        let action = self
            .session
            .world()
            .guidance_for_player(self.player)
            .ok()
            .and_then(|guidance| guidance.recommended_action);
        if let Some(gameplay::GuidanceAction::Interact(target)) = action {
            let mut aggregate = FeedbackAggregate::default();
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
            self.issue_into([GameplayCommand::Interact {
                player: self.player,
                target,
            }], &mut aggregate);
            self.finish_issue(aggregate);
        } else {
            self.feedback = "NO HAY INTERACCION DISPONIBLE".to_owned();
            println!("visual gameplay: {}", self.feedback);
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
            bridge.poll_gamepad();
            let camera = bridge
                .camera(width as f32 / height.max(1) as f32)
                .map_err(|error| format!("cámara visual: {error:?}"))?;
            let mut snapshot = game::render_bridge::snapshot_from_world_with_camera(
                bridge.session.world(),
                Default::default(),
                &camera,
            );
            snapshot.overlay.hud.feedback = Some(bridge.feedback.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(snapshot.nodes.iter().all(|node| {
            node.transform.translation.is_finite()
                && node.transform.rotation.is_finite()
                && node.transform.scale.is_finite()
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
