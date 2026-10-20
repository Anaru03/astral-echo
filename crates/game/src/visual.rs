use bevy::prelude::*;
use engine::{Duration, EntityId, Vec3 as GameplayVec3};
use game::ProjectEchoSession;
use gameplay::types::ItemId;
use gameplay::{
    AttackType, Destination, DestinationId, DestinationType, DiscoveryState, EnemyArchetype,
    EnemyConfiguration, EncounterId, GameplayCommand, GameplayEvent, NavigationRoute,
    NavigationRouteState, SpatialEncounter, SpatialEncounterKind, StationType, WeaponAmmunition,
    WeaponConfiguration,
};

const ORIGIN: DestinationId = DestinationId::new(9_001);
const DESTINATION: DestinationId = DestinationId::new(9_002);
const SIGNAL: DestinationId = DestinationId::new(9_003);
const ENCOUNTER: EncounterId = EncounterId::new(9_004);
const PLAYER_SHIP_ASSET: &str = "models/player_ship.glb";
const ENEMY_SHIP_ASSET: &str = "models/enemy_ship.glb";
const STATION_ASSET: &str = "models/station.glb";
const DERELICT_ASSET: &str = "models/derelict.glb";
const RESOURCE_ASSET: &str = "models/resource.glb";

#[derive(Component)]
struct ShipVisual;

#[derive(Component)]
struct PlayerVisual;

#[derive(Component)]
struct DestinationVisual;

#[derive(Component)]
struct EnemyVisual;

#[derive(Component)]
struct RewardVisual;

#[derive(Component)]
struct HudText;

#[allow(dead_code)]
#[derive(Resource)]
struct OptionalScenes {
    player_ship: Handle<bevy::gltf::Gltf>,
    enemy_ship: Handle<bevy::gltf::Gltf>,
    station: Handle<bevy::gltf::Gltf>,
    derelict: Handle<bevy::gltf::Gltf>,
    resource: Handle<bevy::gltf::Gltf>,
}

struct VisualBridge {
    session: ProjectEchoSession,
    player: EntityId,
    ship: EntityId,
    pilot_station: EntityId,
    enemy: EntityId,
    feedback: String,
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
    }
}

pub fn run() -> Result<(), String> {
    let bridge = VisualBridge::new()?;
    App::new()
        .insert_non_send(bridge)
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".to_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Astral Echo — Visual 3D".to_owned(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, (input_system, sync_world_system, update_hud_system))
        .run();
    Ok(())
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    bridge: NonSend<VisualBridge>,
) {
    commands.insert_resource(OptionalScenes {
        player_ship: asset_server.load(PLAYER_SHIP_ASSET),
        enemy_ship: asset_server.load(ENEMY_SHIP_ASSET),
        station: asset_server.load(STATION_ASSET),
        derelict: asset_server.load(DERELICT_ASSET),
        resource: asset_server.load(RESOURCE_ASSET),
    });
    let ship_material = materials.add(Color::srgb(0.18, 0.75, 1.0));
    let destination_material = materials.add(Color::srgb(0.3, 0.95, 0.65));
    let enemy_material = materials.add(Color::srgb(0.95, 0.18, 0.35));
    let reward_material = materials.add(Color::srgb(1.0, 0.75, 0.18));
    let player_material = materials.add(Color::srgb(0.9, 0.95, 1.0));
    let destination_position = bridge
        .session
        .world()
        .destination(DESTINATION)
        .map(|destination| destination.position)
        .unwrap_or(GameplayVec3::ZERO);
    let enemy_position = bridge
        .session
        .world()
        .enemies()
        .find(|enemy| enemy.entity == bridge.enemy)
        .map(|enemy| enemy.position)
        .unwrap_or(GameplayVec3::ZERO);
    let player_position = bridge
        .session
        .world()
        .player(bridge.player)
        .map(|player| player.position)
        .unwrap_or(GameplayVec3::ZERO);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 1.2, 5.0))),
        MeshMaterial3d(ship_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        ShipVisual,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(4.0))),
        MeshMaterial3d(destination_material),
        Transform::from_translation(to_bevy(destination_position)),
        DestinationVisual,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(1.2, 3.0))),
        MeshMaterial3d(enemy_material),
        Transform::from_translation(to_bevy(enemy_position)),
        EnemyVisual,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.7))),
        MeshMaterial3d(player_material),
        Transform::from_translation(to_bevy(player_position)),
        PlayerVisual,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(reward_material),
        Transform::from_xyz(0.0, -3.0, 0.0),
        Visibility::Hidden,
        RewardVisual,
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 38.0, 62.0).looking_at(Vec3::new(18.0, 0.0, -18.0), Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            intensity: 3_000_000.0,
            range: 180.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 26.0, 18.0),
    ));
    commands.spawn((
        Text::new(hud_text(&bridge)),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
}

fn input_system(keys: Res<ButtonInput<KeyCode>>, mut bridge: NonSendMut<VisualBridge>) {
    let player = bridge.player;
    let ship = bridge.ship;
    let enemy = bridge.enemy;
    let pilot_station = bridge.pilot_station;
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
    if keys.just_pressed(KeyCode::Space) {
        bridge.issue([GameplayCommand::AdvanceTime {
            delta: Duration(100),
        }]);
    }
    if keys.just_pressed(KeyCode::KeyW) {
        bridge.issue([GameplayCommand::MovePlayer {
            player,
            direction: GameplayVec3 { x: 0.0, y: 0.0, z: -1.0 },
            delta: Duration(100),
        }]);
    }
    if keys.just_pressed(KeyCode::KeyA) {
        bridge.issue([GameplayCommand::MovePlayer {
            player,
            direction: GameplayVec3 { x: -1.0, y: 0.0, z: 0.0 },
            delta: Duration(100),
        }]);
    }
    if keys.just_pressed(KeyCode::KeyS) {
        bridge.issue([GameplayCommand::MovePlayer {
            player,
            direction: GameplayVec3 { x: 0.0, y: 0.0, z: 1.0 },
            delta: Duration(100),
        }]);
    }
    if keys.just_pressed(KeyCode::KeyD) {
        bridge.issue([GameplayCommand::MovePlayer {
            player,
            direction: GameplayVec3 { x: 1.0, y: 0.0, z: 0.0 },
            delta: Duration(100),
        }]);
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        bridge.issue([GameplayCommand::ScanSignals {
            player,
            ship,
        }]);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        bridge.issue([GameplayCommand::AttackEnemy {
            player,
            enemy,
        }]);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        let collectible = {
            bridge
                .session
                .world()
                .collectibles()
                .find(|value| value.active)
                .map(|value| (value.entity, value.amount))
        };
        if let Some((collectible, amount)) = collectible {
            bridge.issue([GameplayCommand::CollectItem {
                player,
                collectible,
                amount,
            }]);
        }
    }
    if keys.just_pressed(KeyCode::KeyP) {
        bridge.issue([
            GameplayCommand::Interact {
                player,
                target: pilot_station,
            },
            GameplayCommand::PlanAutopilot {
                player,
                pilot_station,
                origin: ORIGIN,
                destination: DESTINATION,
            },
            GameplayCommand::StartAutopilot {
                player,
                pilot_station,
            },
        ]);
    }
}

fn sync_world_system(
    bridge: NonSend<VisualBridge>,
    mut ship_query: Query<&mut Transform, (With<ShipVisual>, Without<PlayerVisual>, Without<EnemyVisual>, Without<DestinationVisual>, Without<RewardVisual>)>,
    mut player_query: Query<&mut Transform, (With<PlayerVisual>, Without<ShipVisual>, Without<EnemyVisual>, Without<DestinationVisual>, Without<RewardVisual>)>,
    mut enemy_query: Query<(&mut Transform, &mut Visibility), (With<EnemyVisual>, Without<ShipVisual>, Without<PlayerVisual>, Without<DestinationVisual>, Without<RewardVisual>)>,
    mut destination_query: Query<&mut Transform, (With<DestinationVisual>, Without<ShipVisual>, Without<PlayerVisual>, Without<EnemyVisual>, Without<RewardVisual>)>,
    mut reward_query: Query<(&mut Transform, &mut Visibility), (With<RewardVisual>, Without<ShipVisual>, Without<PlayerVisual>, Without<EnemyVisual>, Without<DestinationVisual>)>,
) {
    if let Some(position) = bridge.session.world().ship_position() {
        if let Ok(mut transform) = ship_query.single_mut() {
            transform.translation = to_bevy(position);
        }
    }
    if let Some(player) = bridge.session.world().player(bridge.player) {
        if let Ok(mut transform) = player_query.single_mut() {
            transform.translation = to_bevy(player.position);
        }
    }
    if let Ok((mut transform, mut visibility)) = enemy_query.single_mut() {
        if let Some(enemy) = bridge
            .session
            .world()
            .enemies()
            .find(|value| value.entity == bridge.enemy)
        {
            transform.translation = to_bevy(enemy.position);
            *visibility = if enemy.active && !enemy.is_defeated() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if let Some(destination) = bridge.session.world().destination(DESTINATION) {
        if let Ok(mut transform) = destination_query.single_mut() {
            transform.translation = to_bevy(destination.position);
        }
    }
    if let Ok((mut transform, mut visibility)) = reward_query.single_mut() {
        if let Some(collectible) = bridge.session.world().collectibles().find(|value| value.active) {
            transform.translation = to_bevy(collectible.position);
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn update_hud_system(bridge: NonSend<VisualBridge>, mut query: Query<&mut Text, With<HudText>>) {
    if let Ok(mut text) = query.single_mut() {
        *text = Text::new(hud_text(&bridge));
    }
}

fn hud_text(bridge: &VisualBridge) -> String {
    let world = bridge.session.world();
    let (health, maximum_health, _) = world.player_health(bridge.player).unwrap_or((0, 0, false));
    let (hull, maximum_hull) = world
        .ship_integrity(bridge.ship)
        .map(|value| (value.current(), value.maximum()))
        .unwrap_or((0, 0));
    let (fuel, capacity) = world
        .ship_fuel(bridge.ship)
        .map(|value| (value.current(), value.capacity()))
        .unwrap_or((0, 0));
    let active_enemies = world
        .enemies()
        .filter(|value| value.active && !value.is_defeated())
        .count();
    format!(
        "ASTRAL ECHO 3D\nWASD mover  SPACE tiempo  Q escanear  F atacar  E recoger  P autopilot  ESC salir\nHP {health}/{maximum_health}  CASCO {hull}/{maximum_hull}  COMBUSTIBLE {fuel}/{capacity}\nNAV {:?}  ENEMIGOS {active_enemies}  LOOT {}\n{}",
        world.autopilot_state(), world.collectibles().filter(|value| value.active).count(), bridge.feedback
    )
}

fn to_bevy(position: GameplayVec3) -> Vec3 {
    Vec3::new(position.x, position.y, position.z)
}