use std::cell::RefCell;
use std::rc::Rc;

use engine::{Duration, EntityId, Vec3 as GameplayVec3};
use game::ProjectEchoSession;
use gameplay::types::ItemId;
use gameplay::{
    AttackType, Destination, DestinationId, DestinationType, DiscoveryState, EnemyArchetype,
    EnemyConfiguration, EncounterId, GameplayCommand, GameplayEvent, NavigationRoute,
    NavigationRouteState, SpatialEncounter, SpatialEncounterKind, StationType, WeaponAmmunition,
    WeaponConfiguration,
};
use render_core::{LightingMode, MaterialInfo, SnapshotScene, VisualAssets};
use render_cpu::{Framebuffer, RasterError};
use render_present::{run_window_with_input, KeyCode, Preset, WindowConfig};

const ORIGIN: DestinationId = DestinationId::new(9_001);
const DESTINATION: DestinationId = DestinationId::new(9_002);
const SIGNAL: DestinationId = DestinationId::new(9_003);
const ENCOUNTER: EncounterId = EncounterId::new(9_004);

struct VisualBridge {
    session: ProjectEchoSession,
    player: EntityId,
    ship: EntityId,
    pilot_station: EntityId,
    enemy: EntityId,
    feedback: String,
    scene: SnapshotScene,
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
                    z: -25.0,
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
        Ok(Self {
            session,
            player,
            ship,
            pilot_station,
            enemy,
            feedback: "ESCENA INICIALIZADA".to_owned(),
            scene: visual_scene(),
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

    fn handle_key(&mut self, key: KeyCode) {
        let player = self.player;
        let ship = self.ship;
        let enemy = self.enemy;
        let pilot_station = self.pilot_station;
        match key {
            KeyCode::Space => self.issue([GameplayCommand::AdvanceTime {
                delta: Duration(100),
            }]),
            KeyCode::KeyW => self.issue([GameplayCommand::MovePlayer {
                player,
                direction: GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: -1.0,
                },
                delta: Duration(100),
            }]),
            KeyCode::KeyA => self.issue([GameplayCommand::MovePlayer {
                player,
                direction: GameplayVec3 {
                    x: -1.0,
                    y: 0.0,
                    z: 0.0,
                },
                delta: Duration(100),
            }]),
            KeyCode::KeyS => self.issue([GameplayCommand::MovePlayer {
                player,
                direction: GameplayVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                delta: Duration(100),
            }]),
            KeyCode::KeyD => self.issue([GameplayCommand::MovePlayer {
                player,
                direction: GameplayVec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                delta: Duration(100),
            }]),
            KeyCode::KeyQ => self.issue([GameplayCommand::ScanSignals { player, ship }]),
            KeyCode::KeyF => self.issue([GameplayCommand::AttackEnemy { player, enemy }]),
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
            _ => {}
        }
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
}

pub fn run() -> Result<(), String> {
    let runtime = Rc::new(RefCell::new(VisualBridge::new()?));
    let render_runtime = Rc::clone(&runtime);
    let input_runtime = Rc::clone(&runtime);
    let config = WindowConfig {
        title: "Astral Echo — CPU propio".into(),
        preset: Preset::Low,
        raster_controls: false,
        ..Default::default()
    };
    run_window_with_input(
        config,
        move |framebuffer: &mut Framebuffer, mode| {
            render_frame(&mut render_runtime.borrow_mut(), framebuffer, mode)
        },
        move |key| input_runtime.borrow_mut().handle_key(key),
    )
    .map(|report| {
        println!(
            "CPU visual finalizado: frames={} render_avg_ms={:.3} present_avg_ms={:.3}",
            report.metrics.frames,
            report.metrics.average_render().as_secs_f64() * 1000.0,
            report.metrics.average_presentation().as_secs_f64() * 1000.0,
        );
    })
    .map_err(|error| format!("ventana visual CPU: {error}"))
}

fn render_frame(
    bridge: &mut VisualBridge,
    framebuffer: &mut Framebuffer,
    mode: render_cpu::RasterMode,
) -> Result<(), RasterError> {
    let camera = game::render_bridge::flight_camera(
        bridge.session.world(),
        framebuffer.width() as f32 / framebuffer.height() as f32,
    )
        .map_err(|_| RasterError::InvalidViewport)?;
    let snapshot = game::render_bridge::snapshot_from_world_with_camera(
        bridge.session.world(),
        Default::default(),
        &camera,
    );
    bridge
        .scene
        .update(&snapshot)
        .map_err(|_| RasterError::NonFiniteVertex)?;
    render_cpu::render_snapshot_scene(framebuffer, &bridge.scene, mode).map(|_| ())
}

fn visual_scene() -> SnapshotScene {
    let mut scene = SnapshotScene::default();
    scene.set_lighting_mode(LightingMode::Lambert);
    scene.set_material(VisualAssets::SUN, MaterialInfo::sun());
    scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
    scene
}