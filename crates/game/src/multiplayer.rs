use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use engine::{Duration, EntityId, Vec3};
use gameplay::{CommandStatus, GameplayCommand};
use networking::{
    GameServer, GameServerHandle, NetworkOrientation, NetworkPlayerId, NetworkVec3,
    Mission5ContentProjection, Mission5DestinationProjection, Mission5ObjectiveProjection,
    Mission5DestinationTypeProjection, Mission5DiscoveryProjection, Mission5ShooterProjection,
    MissionRecoveryOutcome, MissionStateProjection, ObjectiveStateProjection, PlayerTransform,
    EnemyProjection, ServerEvent, ServerEventQueue, ServerMessage, InventoryItemProjection,
    SharedInventoryProjection,
};

use crate::ProjectEchoSession;

const NETWORK_TICK: Duration = Duration(100);
const MAX_RECOVERY_REQUESTS_PER_PLAYER: usize = 32;
const PLAYER_COLLISION_DIAMETER: f32 = 1.2;

#[derive(Debug)]
pub enum MultiplayerError {
    Io(io::Error),
    PlayerCreation(gameplay::PlayerCreationError),
    UnknownPlayer(NetworkPlayerId),
    InvalidMovement(NetworkPlayerId),
    GameplayRejected(NetworkPlayerId),
    ServerThread,
}

impl std::fmt::Display for MultiplayerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "error de red: {error}"),
            Self::PlayerCreation(error) => {
                write!(formatter, "no se pudo crear el jugador: {error:?}")
            }
            Self::UnknownPlayer(player) => {
                write!(formatter, "jugador de red desconocido: {player}")
            }
            Self::InvalidMovement(player) => {
                write!(formatter, "movimiento inválido para el jugador {player}")
            }
            Self::GameplayRejected(player) => {
                write!(formatter, "gameplay rechazó el movimiento de {player}")
            }
            Self::ServerThread => write!(
                formatter,
                "el servidor multijugador terminó inesperadamente"
            ),
        }
    }
}

impl std::error::Error for MultiplayerError {}

impl From<io::Error> for MultiplayerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkPlayer {
    pub network_id: NetworkPlayerId,
    pub entity: EntityId,
}

pub struct MultiplayerSession {
    gameplay: ProjectEchoSession,
    server: GameServerHandle,
    address: std::net::SocketAddr,
    events: ServerEventQueue,
    players: BTreeMap<NetworkPlayerId, EntityId>,
    recovery_results: BTreeMap<NetworkPlayerId, BTreeMap<u64, MissionRecoveryOutcome>>,
    shutdown: Arc<AtomicBool>,
    server_thread: Option<JoinHandle<io::Result<()>>>,
}

impl MultiplayerSession {
    pub fn bind(address: &str) -> Result<Self, MultiplayerError> {
        let server = GameServer::bind(address)?;
        let local_address = server.local_addr()?;
        let events = server.event_queue();
        let server_handle = server.handle_with_events(events.clone());
        let shutdown = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&shutdown);
        let server_events = events.clone();
        let server_thread =
            thread::spawn(move || server.run_until_with_events(&signal, Some(server_events)));

        let mut gameplay = ProjectEchoSession::new();
        gameplay
            .world_mut()
            .enable_shared_inventory()
            .map_err(|_| MultiplayerError::GameplayRejected(0))?;
        Ok(Self {
            gameplay,
            server: server_handle,
            address: local_address,
            events,
            players: BTreeMap::new(),
            recovery_results: BTreeMap::new(),
            shutdown,
            server_thread: Some(server_thread),
        })
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, MultiplayerError> {
        Ok(self.address)
    }

    pub fn gameplay(&self) -> &ProjectEchoSession {
        &self.gameplay
    }

    pub fn network_player(&self, player_id: NetworkPlayerId) -> Option<NetworkPlayer> {
        self.players
            .get(&player_id)
            .copied()
            .map(|entity| NetworkPlayer {
                network_id: player_id,
                entity,
            })
    }

    pub fn update(&mut self) -> Result<usize, MultiplayerError> {
        let mut processed = 0;
        while let Some(event) = self.events.pop() {
            processed += 1;
            match event {
                ServerEvent::PlayerJoined { player_id, .. } => self.register_player(player_id)?,
                ServerEvent::PlayerMoved {
                    player_id,
                    sequence,
                    transform,
                } => self.apply_movement(player_id, sequence, transform)?,
                ServerEvent::PlayerInteracted {
                    player_id,
                    request_id,
                    target,
                } => self.apply_interaction(player_id, request_id, target),
                ServerEvent::PlayerAttacked {
                    player_id,
                    request_id,
                    enemy,
                } => self.apply_attack(player_id, request_id, enemy),
                ServerEvent::PlayerCollected {
                    player_id,
                    request_id,
                    collectible,
                    amount,
                } => self.apply_collect(player_id, request_id, collectible, amount),
                ServerEvent::PlayerEquippedWeapon {
                    player_id,
                    request_id,
                    item,
                } => self.apply_equip_weapon(player_id, request_id, item),
                ServerEvent::PlayerUsedHealingItem {
                    player_id,
                    request_id,
                    item,
                    amount,
                } => self.apply_healing_item(player_id, request_id, item, amount),
                ServerEvent::PlayerRequestedTravel {
                    player_id,
                    request_id,
                    pilot_station,
                    destination,
                } => self.apply_travel(player_id, request_id, pilot_station, destination),
                ServerEvent::MissionRecoveryRequested {
                    player_id,
                    request_id,
                } => self.handle_mission_recovery(player_id, request_id),
                ServerEvent::PlayerLeft { player_id } => self.unregister_player(player_id),
            }
        }
        if self
            .server_thread
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            return Err(MultiplayerError::ServerThread);
        }
        Ok(processed)
    }

    fn register_player(&mut self, player_id: NetworkPlayerId) -> Result<(), MultiplayerError> {
        if self.players.contains_key(&player_id) {
            return Ok(());
        }

        // Buscar una entidad inactiva que no esté asignada
        // actualmente a otro jugador de red.
        let reusable = self
            .gameplay
            .world()
            .players()
            .find(|player| {
                !player.is_active
                    && !self
                        .players
                        .values()
                        .any(|&entity| entity == player.entity_id)
            })
            .map(|player| player.entity_id);

        // La sesión de gameplay ya crea un jugador inicial activo.
        // Asignarlo a la primera conexión en lugar de crear
        // una cuarta entidad innecesaria.
        let initial_player = self.gameplay.ids.first_player;

        let initial_available =
            self.players.is_empty() && self.gameplay.world().player(initial_player).is_some();

        let entity = if let Some(entity) = reusable {
            let result = self.gameplay.update([GameplayCommand::SetPlayerActive {
                player: entity,
                active: true,
            }]);

            if !matches!(
                result.first().map(|value| value.status),
                Some(CommandStatus::Success { .. })
            ) {
                return Err(MultiplayerError::GameplayRejected(player_id));
            }

            entity
        } else if initial_available {
            initial_player
        } else {
            self.gameplay
                .world_mut()
                .create_player()
                .map_err(MultiplayerError::PlayerCreation)?
        };

        if self.gameplay.world().inventory(entity).is_none() {
            self.gameplay
                .world_mut()
                .initialize_inventory(entity, 20)
                .map_err(|_| MultiplayerError::GameplayRejected(player_id))?;
        }
        self.players.insert(player_id, entity);

        if let Some(mut player) = self.gameplay.world().player(entity).copied() {
            let slot = self.players.keys().position(|id| *id == player_id).unwrap_or(0) as f32;
            player.position.x += slot * PLAYER_COLLISION_DIAMETER * 1.15;
            self.gameplay
                .world_mut()
                .initialize_player_position(entity, player.position);
        }

        let _ = self.server.send_to(
            player_id,
            &ServerMessage::Mission5ContentState {
                content: self.mission5_content_projection(),
            },
        );
        let _ = self.server.send_to(
            player_id,
            &ServerMessage::SharedInventoryUpdated {
                inventory: self.shared_inventory_projection(),
            },
        );
        let _ = self.server.send_to(
            player_id,
            &ServerMessage::EnemyStateUpdated {
                enemies: self.enemy_state_projection(),
            },
        );

        let transforms = self
            .players
            .iter()
            .filter_map(|(&network_id, &entity)| {
                let player = self.gameplay.world().player(entity)?;
                Some((
                    network_id,
                    PlayerTransform::new(
                        NetworkVec3::new(
                            player.position.x,
                            player.position.y,
                            player.position.z,
                        ),
                        NetworkOrientation::new(
                            player.orientation.yaw_degrees,
                            player.orientation.pitch_degrees,
                        ),
                        NetworkVec3::new(
                            player.velocity.x,
                            player.velocity.y,
                            player.velocity.z,
                        ),
                    ),
                ))
            })
            .collect::<Vec<_>>();
        for &recipient in self.players.keys() {
            for &(remote_id, transform) in &transforms {
                let _ = self.server.send_to(
                    recipient,
                    &ServerMessage::PlayerMoved {
                        player_id: remote_id,
                        sequence: 0,
                        transform,
                    },
                );
            }
        }

        Ok(())
    }

    fn apply_interaction(
        &mut self,
        player_id: NetworkPlayerId,
        request_id: u64,
        target: u64,
    ) {
        let result = self.players.get(&player_id).copied().map(|player| {
            self.gameplay.update([GameplayCommand::Interact {
                player,
                target: EntityId::new(target),
            }])
        });
        self.publish_action_result(player_id, request_id, result.and_then(|results| results.into_iter().next()));
    }

    fn apply_attack(&mut self, player_id: NetworkPlayerId, request_id: u64, enemy: u64) {
        let result = self.players.get(&player_id).copied().map(|player| {
            self.gameplay.update([GameplayCommand::AttackEnemy {
                player,
                enemy: EntityId::new(enemy),
            }])
        });
        self.publish_action_result(player_id, request_id, result.and_then(|results| results.into_iter().next()));
    }

    fn apply_collect(
        &mut self,
        player_id: NetworkPlayerId,
        request_id: u64,
        collectible: u64,
        amount: u32,
    ) {
        let result = self.players.get(&player_id).copied().map(|player| {
            self.gameplay.update([GameplayCommand::CollectItem {
                player,
                collectible: EntityId::new(collectible),
                amount,
            }])
        });
        self.publish_action_result(player_id, request_id, result.and_then(|results| results.into_iter().next()));
    }

    fn publish_action_result(
        &mut self,
        player_id: NetworkPlayerId,
        request_id: u64,
        result: Option<gameplay::CommandResult>,
    ) {
        let (accepted, reason) = match result {
            Some(result) => match result.status {
                CommandStatus::Success { .. } => (true, None),
                CommandStatus::Rejected { reason } => (false, Some(format!("{reason:?}"))),
                CommandStatus::Invalid { reason } => (false, Some(format!("{reason:?}"))),
            },
            None => (false, Some("UnknownPlayer".to_owned())),
        };
        let _ = self.server.send_to(
            player_id,
            &ServerMessage::GameplayActionResult {
                request_id,
                accepted,
                reason,
            },
        );
        if accepted {
            self.server.broadcast(&ServerMessage::MissionContentUpdated {
                content: self.mission5_content_projection(),
            });
            self.broadcast_shared_inventory();
            self.broadcast_enemy_state();
        }
    }

    fn shared_inventory_projection(&self) -> SharedInventoryProjection {
        let inventory = self
            .gameplay
            .world()
            .players()
            .next()
            .and_then(|player| self.gameplay.world().inventory(player.entity_id));
        SharedInventoryProjection {
            capacity: inventory.map_or(0, |value| value.capacity()),
            entries: inventory
                .into_iter()
                .flat_map(|value| value.entries())
                .filter(|(_, amount)| *amount > 0)
                .map(|(item, amount)| InventoryItemProjection {
                    item: item.0,
                    amount,
                })
                .collect(),
        }
    }

    fn broadcast_shared_inventory(&self) {
        self.server.broadcast(&ServerMessage::SharedInventoryUpdated {
            inventory: self.shared_inventory_projection(),
        });
    }

    fn enemy_state_projection(&self) -> Vec<EnemyProjection> {
        self.gameplay
            .world()
            .enemies()
            .map(|enemy| EnemyProjection {
                entity_id: enemy.entity.value(),
                active: enemy.active,
                current_health: enemy.current_health,
                maximum_health: enemy.maximum_health,
                position: [enemy.position.x, enemy.position.y, enemy.position.z],
            })
            .collect()
    }

    fn broadcast_enemy_state(&self) {
        self.server.broadcast(&ServerMessage::EnemyStateUpdated {
            enemies: self.enemy_state_projection(),
        });
    }

    fn apply_equip_weapon(&mut self, player_id: NetworkPlayerId, request_id: u64, item: u32) {
        let result = self.players.get(&player_id).copied().map(|player| {
            self.gameplay.update([GameplayCommand::EquipPlayerWeaponItem {
                player,
                item: gameplay::types::ItemId(item),
            }])
        });
        self.publish_action_result(player_id, request_id, result.and_then(|results| results.into_iter().next()));
    }

    fn apply_healing_item(
        &mut self,
        player_id: NetworkPlayerId,
        request_id: u64,
        item: u32,
        amount: u32,
    ) {
        let result = self.players.get(&player_id).copied().map(|player| {
            self.gameplay.update([GameplayCommand::UseHealingConsumable {
                player,
                item: gameplay::types::ItemId(item),
                amount,
            }])
        });
        self.publish_action_result(player_id, request_id, result.and_then(|results| results.into_iter().next()));
    }

    fn apply_travel(
        &mut self,
        player_id: NetworkPlayerId,
        request_id: u64,
        pilot_station: u64,
        destination: u32,
    ) {
        let result = self.players.get(&player_id).copied().map(|player| {
            let results = self.gameplay.update([
                GameplayCommand::SelectDestination {
                    player,
                    pilot_station: EntityId::new(pilot_station),
                    destination: gameplay::DestinationId::new(destination),
                },
                GameplayCommand::StartTravel {
                    player,
                    pilot_station: EntityId::new(pilot_station),
                },
            ]);
            results
                .iter()
                .find(|result| !matches!(result.status, CommandStatus::Success { .. }))
                .copied()
                .or_else(|| results.last().copied())
        });
        self.publish_action_result(player_id, request_id, result.flatten());
    }

    fn unregister_player(&mut self, player_id: NetworkPlayerId) {
        self.recovery_results.remove(&player_id);
        if let Some(entity) = self.players.remove(&player_id) {
            let _ = self.gameplay.update([GameplayCommand::SetPlayerActive {
                player: entity,
                active: false,
            }]);
        }
    }

    fn handle_mission_recovery(&mut self, player_id: NetworkPlayerId, request_id: u64) {
        if let Some(outcome) = self
            .recovery_results
            .get(&player_id)
            .and_then(|results| results.get(&request_id))
            .cloned()
        {
            let _ = self.server.send_to(
                player_id,
                &ServerMessage::MissionRecoveryResult {
                    request_id,
                    outcome,
                },
            );
            return;
        }

        let outcome = if request_id == 0 {
            MissionRecoveryOutcome::InvalidRequest
        } else if !self.players.contains_key(&player_id) {
            MissionRecoveryOutcome::Unauthorized
        } else {
            let result = self
                .gameplay
                .update([GameplayCommand::RetryCooperativeMissionContent]);
            match result.first().map(|value| value.status) {
                Some(CommandStatus::Success { .. }) => MissionRecoveryOutcome::Succeeded,
                Some(CommandStatus::Rejected {
                    reason: gameplay::CommandRejection::MissionContentUnavailable,
                }) => MissionRecoveryOutcome::ContentUnavailable,
                Some(CommandStatus::Rejected {
                    reason: gameplay::CommandRejection::InvalidMissionTransition,
                }) => MissionRecoveryOutcome::InvalidMissionTransition,
                _ => MissionRecoveryOutcome::SessionUnavailable,
            }
        };

        if self.players.contains_key(&player_id) {
            let results = self.recovery_results.entry(player_id).or_default();
            if results.len() >= MAX_RECOVERY_REQUESTS_PER_PLAYER {
                if let Some(first) = results.keys().next().copied() {
                    results.remove(&first);
                }
            }
            results.insert(request_id, outcome.clone());
        }

        let _ = self.server.send_to(
            player_id,
            &ServerMessage::MissionRecoveryResult {
                request_id,
                outcome: outcome.clone(),
            },
        );

        if matches!(outcome, MissionRecoveryOutcome::Succeeded) {
            self.server.broadcast(&ServerMessage::MissionContentUpdated {
                content: self.mission5_content_projection(),
            });
        }
    }

    fn mission5_content_projection(&self) -> Mission5ContentProjection {
        let mission_id = crate::cooperacion::PROGRESION_COOPERATIVA;
        let mission = self
            .gameplay
            .world()
            .mission(mission_id)
            .expect("Mission5 debe estar registrada");
        let progress = mission.progress();
        let state = match progress.state {
            gameplay::domains::mission::MissionState::Locked => MissionStateProjection::Locked,
            gameplay::domains::mission::MissionState::Available => {
                MissionStateProjection::Available
            }
            gameplay::domains::mission::MissionState::Active => MissionStateProjection::Active,
            gameplay::domains::mission::MissionState::Failed => MissionStateProjection::Failed,
            gameplay::domains::mission::MissionState::Completed => {
                MissionStateProjection::Completed
            }
        };
        let objectives = mission
            .objectives()
            .iter()
            .map(|objective| Mission5ObjectiveProjection {
                objective_id: objective.id.0,
                required: objective.required,
                state: match objective.state() {
                    gameplay::domains::objective::ObjectiveState::Locked => {
                        ObjectiveStateProjection::Locked
                    }
                    gameplay::domains::objective::ObjectiveState::Available => {
                        ObjectiveStateProjection::Available
                    }
                    gameplay::domains::objective::ObjectiveState::Active => {
                        ObjectiveStateProjection::Active
                    }
                    gameplay::domains::objective::ObjectiveState::Failed => {
                        ObjectiveStateProjection::Failed
                    }
                    gameplay::domains::objective::ObjectiveState::Completed => {
                        ObjectiveStateProjection::Completed
                    }
                },
                progress: objective.progress(),
            })
            .collect();
        let destination = self
            .gameplay
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .map(|value| Mission5DestinationProjection {
                destination_id: value.id.value(),
                destination_type: match value.destination_type {
                    gameplay::DestinationType::Planet => Mission5DestinationTypeProjection::Planet,
                    gameplay::DestinationType::SpacePoint => {
                        Mission5DestinationTypeProjection::SpacePoint
                    }
                },
                position: [value.position.x, value.position.y, value.position.z],
                travel_duration: value.travel_duration.0,
                fuel_cost: value.fuel_cost,
                discovery: match value.discovery {
                    gameplay::DiscoveryState::Unknown => Mission5DiscoveryProjection::Unknown,
                    gameplay::DiscoveryState::Known => Mission5DiscoveryProjection::Known,
                    gameplay::DiscoveryState::Discovered => {
                        Mission5DiscoveryProjection::Discovered
                    }
                },
            });
        let shooter = self
            .gameplay
            .world()
            .combatants()
            .find(|value| {
                !value.is_player
                    && value.active
                    && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
            })
            .map(|value| Mission5ShooterProjection {
                entity_id: value.entity.value(),
                active: value.active,
                current_health: value.current_health,
                maximum_health: value.maximum_health,
            });

        Mission5ContentProjection {
            mission_id: mission_id.0,
            state,
            required_completed: progress.required_completed as u32,
            required_total: progress.required_total as u32,
            optional_completed: progress.optional_completed as u32,
            optional_total: progress.optional_total as u32,
            objectives,
            destination,
            shooter,
        }
    }

    fn apply_movement(
        &mut self,
        player_id: NetworkPlayerId,
        sequence: u64,
        transform: PlayerTransform,
    ) -> Result<(), MultiplayerError> {
        let entity = self
            .players
            .get(&player_id)
            .copied()
            .ok_or(MultiplayerError::UnknownPlayer(player_id))?;
        let velocity = to_gameplay_vec3(transform.velocity);
        if !(velocity.x.is_finite() && velocity.y.is_finite() && velocity.z.is_finite())
            || velocity.x * velocity.x + velocity.y * velocity.y + velocity.z * velocity.z > 1.0001
            || !transform.orientation.yaw_degrees.is_finite()
            || !transform.orientation.pitch_degrees.is_finite()
        {
            return Ok(());
        }
        let player = self
            .gameplay
            .world()
            .player(entity)
            .copied()
            .ok_or(MultiplayerError::UnknownPlayer(player_id))?;
        let yaw_delta = transform.orientation.yaw_degrees - player.orientation.yaw_degrees;
        let pitch_delta = transform.orientation.pitch_degrees - player.orientation.pitch_degrees;
        if !yaw_delta.is_finite()
            || !pitch_delta.is_finite()
            || yaw_delta.abs() > 360.0
            || pitch_delta.abs() > 360.0
        {
            return Ok(());
        }
        let results = self.gameplay.update([
            GameplayCommand::MovePlayer {
                player: entity,
                direction: velocity,
                delta: NETWORK_TICK,
            },
            GameplayCommand::RotatePlayer {
                player: entity,
                yaw_delta_degrees: yaw_delta,
                pitch_delta_degrees: pitch_delta,
            },
        ]);
        if results
            .iter()
            .any(|result| !matches!(result.status, CommandStatus::Success { .. }))
        {
            return Ok(());
        }
        let mut updated = self
            .gameplay
            .world()
            .player(entity)
            .copied()
            .ok_or(MultiplayerError::UnknownPlayer(player_id))?;
        for (&other_id, &other_entity) in &self.players {
            if other_id == player_id {
                continue;
            }
            let Some(other) = self.gameplay.world().player(other_entity) else {
                continue;
            };
            let dx = updated.position.x - other.position.x;
            let dz = updated.position.z - other.position.z;
            let distance_squared = dx * dx + dz * dz;
            let minimum_squared = PLAYER_COLLISION_DIAMETER * PLAYER_COLLISION_DIAMETER;
            if distance_squared >= minimum_squared {
                continue;
            }
            let (nx, nz) = if distance_squared > 0.0001 {
                let inverse_distance = distance_squared.sqrt().recip();
                (dx * inverse_distance, dz * inverse_distance)
            } else if player_id < other_id {
                (-1.0, 0.0)
            } else {
                (1.0, 0.0)
            };
            updated.position.x = other.position.x + nx * PLAYER_COLLISION_DIAMETER;
            updated.position.z = other.position.z + nz * PLAYER_COLLISION_DIAMETER;
            self.gameplay
                .world_mut()
                .initialize_player_position(entity, updated.position);
        }
        self.server.broadcast(
            &ServerMessage::PlayerMoved {
                player_id,
                sequence,
                transform: PlayerTransform::new(
                    NetworkVec3::new(updated.position.x, updated.position.y, updated.position.z),
                    NetworkOrientation::new(
                        updated.orientation.yaw_degrees,
                        updated.orientation.pitch_degrees,
                    ),
                    NetworkVec3::new(updated.velocity.x, updated.velocity.y, updated.velocity.z),
                ),
            },
        );
        Ok(())
    }
}

impl Drop for MultiplayerSession {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.server_thread.take() {
            let _ = thread.join();
        }
    }
}

fn to_gameplay_vec3(value: NetworkVec3) -> Vec3 {
    Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use networking::{ClientMessage, GameClient};
    use std::thread;
    use std::time::Duration as StdDuration;

    fn wait_for_update(session: &mut MultiplayerSession) {
        for _ in 0..100 {
            if session.update().unwrap() > 0 {
                return;
            }
            thread::sleep(StdDuration::from_millis(5));
        }
        panic!("no se recibió el evento multijugador");
    }

    fn wait_for_player(session: &mut MultiplayerSession, player_id: NetworkPlayerId) {
        for _ in 0..100 {
            session.update().unwrap();
            if session.network_player(player_id).is_some() {
                return;
            }
            thread::sleep(StdDuration::from_millis(5));
        }
        panic!("no se registró el jugador de red");
    }

    fn receive_initial_mission5_state(client: &mut GameClient) -> Mission5ContentProjection {
        for _ in 0..8 {
            match client.receive().unwrap() {
                ServerMessage::Mission5ContentState { content } => return content,
                _ => {}
            }
        }
        panic!("no se recibió el estado inicial de Mission5");
    }

    fn receive_non_bootstrap(client: &mut GameClient) -> ServerMessage {
        loop {
            let message = client.receive().unwrap();
            match message {
                ServerMessage::SharedInventoryUpdated { .. }
                | ServerMessage::EnemyStateUpdated { .. }
                | ServerMessage::PilotControlChanged { .. }
                | ServerMessage::ShipMoved { sequence: 0, .. }
                | ServerMessage::PlayerMoved { sequence: 0, .. } => continue,
                other => return other,
            }
        }
    }

    fn drain_initial_sync(client: &mut GameClient) {
        while let Some(message) = client.try_receive().unwrap() {
            assert!(matches!(
                message,
                ServerMessage::SharedInventoryUpdated { .. }
                    | ServerMessage::EnemyStateUpdated { .. }
                    | ServerMessage::PlayerMoved { sequence: 0, .. }
            ));
        }
    }

    #[test]
    fn ingreso_crea_una_entidad_estable_y_movimiento_se_aplica() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut client = GameClient::connect(address, "Jugador").unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut client),
            ServerMessage::Mission5ContentState { .. }
        ));
        let network_player = session.network_player(client.player_id()).unwrap();
        let initial = session
            .gameplay()
            .world()
            .player(network_player.entity)
            .unwrap()
            .position;
        client
            .send(&ClientMessage::PlayerMove {
                sequence: 1,
                transform: PlayerTransform::new(
                    NetworkVec3::new(999.0, 999.0, 999.0),
                    NetworkOrientation::new(30.0, 0.0),
                    NetworkVec3::new(1.0, 0.0, 0.0),
                ),
            })
            .unwrap();
        wait_for_update(&mut session);
        let moved = session
            .gameplay()
            .world()
            .player(network_player.entity)
            .unwrap()
            .position;
        assert_ne!(initial, moved);
        client.disconnect().unwrap();
    }

    #[test]
    fn nuevo_cliente_recibe_mission5_sin_contenido_sin_mutar_el_mundo() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut client = GameClient::connect(address, "Jugador").unwrap();
        wait_for_update(&mut session);

        let message = receive_non_bootstrap(&mut client);
        let ServerMessage::Mission5ContentState { content } = message else {
            panic!("faltó el estado inicial de Mission5");
        };
        assert_eq!(content, session.mission5_content_projection());
        assert_eq!(content.mission_id, 5);
        assert!(content.destination.is_none());
        assert!(content.shooter.is_none());
        assert_eq!(client.mission5_content(), Some(&content));
        assert!(session
            .gameplay()
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_none());
        assert!(!session.gameplay().world().combatants().any(|value| {
            !value.is_player
                && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
        }));

        client.disconnect().unwrap();
    }

    #[test]
    fn los_tres_jugadores_reciben_inventario_al_conectarse() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut clients = Vec::new();
        for name in ["Jugador 1", "Jugador 2", "Jugador 3"] {
            let client = GameClient::connect(address, name).unwrap();
            wait_for_update(&mut session);
            clients.push(client);
        }

        for client in &clients {
            let entity = session.network_player(client.player_id()).unwrap().entity;
            assert_eq!(session.gameplay().world().inventory(entity).map(|value| value.capacity()), Some(20));
        }

        for mut client in clients {
            client.disconnect().unwrap();
        }
    }

    #[test]
    fn nuevo_cliente_recibe_estado_actual_de_mission5_sin_replay_historico() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut first = GameClient::connect(address, "Jugador 1").unwrap();
        wait_for_update(&mut session);
        let _ = receive_non_bootstrap(&mut first);

        let mission = crate::cooperacion::PROGRESION_COOPERATIVA;
        let setup = session.gameplay.update([
            GameplayCommand::UnlockMission { mission },
            GameplayCommand::StartMission { mission },
        ]);
        assert!(setup.iter().all(|result| matches!(
            result.status,
            CommandStatus::Success { .. }
        )));

        first.request_mission_recovery(21).unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut first),
            ServerMessage::MissionRecoveryResult { .. }
        ));
        assert!(matches!(
            receive_non_bootstrap(&mut first),
            ServerMessage::MissionContentUpdated { .. }
        ));

        let mut second = GameClient::connect(address, "Jugador 2").unwrap();
        wait_for_player(&mut session, second.player_id());
        assert!(matches!(
            receive_non_bootstrap(&mut second),
            ServerMessage::PlayerJoined { .. }
        ));
        let message = receive_non_bootstrap(&mut second);
        let ServerMessage::Mission5ContentState { content } = message else {
            panic!("faltó el estado inicial para el segundo cliente");
        };
        assert_eq!(content, session.mission5_content_projection());
        assert!(content.destination.is_some());
        assert!(content.shooter.is_some());
        assert_eq!(second.mission5_content(), Some(&content));
        drain_initial_sync(&mut second);
        assert!(second.try_receive().unwrap().is_none());

        first.disconnect().unwrap();
        second.disconnect().unwrap();
    }

    #[test]
    fn recuperacion_de_mission5_devuelve_resultado_autoritativo_correlacionado() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut client = GameClient::connect(address, "Jugador").unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut client),
            ServerMessage::Mission5ContentState { .. }
        ));

        client.request_mission_recovery(7).unwrap();
        wait_for_update(&mut session);
        assert_eq!(
            receive_non_bootstrap(&mut client),
            ServerMessage::MissionRecoveryResult {
                request_id: 7,
                outcome: networking::MissionRecoveryOutcome::InvalidMissionTransition,
            }
        );
        assert!(client.try_receive().unwrap().is_none());

        client.request_mission_recovery(7).unwrap();
        wait_for_update(&mut session);
        assert_eq!(
            receive_non_bootstrap(&mut client),
            ServerMessage::MissionRecoveryResult {
                request_id: 7,
                outcome: networking::MissionRecoveryOutcome::InvalidMissionTransition,
            }
        );
        client.disconnect().unwrap();
    }

    #[test]
    fn recuperacion_exitosa_replicada_a_todos_sin_repetir_broadcast() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut first = GameClient::connect(address, "Jugador 1").unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut first),
            ServerMessage::Mission5ContentState { .. }
        ));
        let mut second = GameClient::connect(address, "Jugador 2").unwrap();
        wait_for_player(&mut session, second.player_id());

        assert!(matches!(
            receive_non_bootstrap(&mut first),
            ServerMessage::PlayerJoined { .. }
        ));
        assert!(matches!(
            receive_non_bootstrap(&mut second),
            ServerMessage::PlayerJoined { .. }
        ));
        assert!(matches!(
            receive_non_bootstrap(&mut second),
            ServerMessage::Mission5ContentState { .. }
        ));

        let mission = crate::cooperacion::PROGRESION_COOPERATIVA;
        let setup = session.gameplay.update([
            GameplayCommand::UnlockMission { mission },
            GameplayCommand::StartMission { mission },
        ]);
        assert!(setup.iter().all(|result| matches!(
            result.status,
            CommandStatus::Success { .. }
        )));

        first.request_mission_recovery(11).unwrap();
        wait_for_update(&mut session);

        assert_eq!(
            receive_non_bootstrap(&mut first),
            ServerMessage::MissionRecoveryResult {
                request_id: 11,
                outcome: networking::MissionRecoveryOutcome::Succeeded,
            }
        );
        let first_update = receive_non_bootstrap(&mut first);
        let second_update = receive_non_bootstrap(&mut second);
        assert_eq!(first_update, second_update);
        let ServerMessage::MissionContentUpdated { content } = first_update else {
            panic!("faltó la actualización compartida de Mission5");
        };
        assert_eq!(content.mission_id, 5);
        assert_eq!(content.required_total, 3);
        assert!(content.destination.is_some());
        assert!(content.shooter.is_some());
        assert!(content.shooter.as_ref().is_some_and(|shooter| shooter.active));
        assert_eq!(first.mission5_content(), Some(&content));

        first.request_mission_recovery(11).unwrap();
        wait_for_update(&mut session);
        assert_eq!(
            receive_non_bootstrap(&mut first),
            ServerMessage::MissionRecoveryResult {
                request_id: 11,
                outcome: networking::MissionRecoveryOutcome::Succeeded,
            }
        );
        assert!(first.try_receive().unwrap().is_none());
        assert!(second.try_receive().unwrap().is_none());

        first.disconnect().unwrap();
        second.disconnect().unwrap();
    }

    #[test]
    fn proyeccion_mission5_selecciona_el_shooter_activo_preparado() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut client = GameClient::connect(address, "Jugador").unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut client),
            ServerMessage::Mission5ContentState { .. }
        ));

        let create_result = session.gameplay.update([GameplayCommand::CreateEnemy {
            configuration: gameplay::EnemyConfiguration::for_archetype(
                gameplay::EnemyArchetype::Shooter,
                Vec3::ZERO,
            ),
        }]);
        assert!(matches!(
            create_result.first().map(|result| result.status),
            Some(CommandStatus::Success { changed: true })
        ));
        let inactive_id = session
            .gameplay
            .world()
            .combatants()
            .find(|combatant| {
                !combatant.is_player
                    && combatant.archetype == Some(gameplay::EnemyArchetype::Shooter)
            })
            .map(|combatant| combatant.entity)
            .expect("debe existir el Shooter preexistente");
        let deactivate_result = session.gameplay.update([GameplayCommand::SetEnemyActive {
            enemy: inactive_id,
            active: false,
        }]);
        assert!(matches!(
            deactivate_result.first().map(|result| result.status),
            Some(CommandStatus::Success { changed: true })
        ));

        let mission = crate::cooperacion::PROGRESION_COOPERATIVA;
        let setup = session.gameplay.update([
            GameplayCommand::UnlockMission { mission },
            GameplayCommand::StartMission { mission },
        ]);
        assert!(setup.iter().all(|result| matches!(
            result.status,
            CommandStatus::Success { .. }
        )));

        let active_id = session
            .gameplay
            .world()
            .combatants()
            .find(|combatant| {
                !combatant.is_player
                    && combatant.active
                    && combatant.archetype == Some(gameplay::EnemyArchetype::Shooter)
            })
            .map(|combatant| combatant.entity)
            .expect("la preparación debe crear un Shooter activo");
        assert_ne!(inactive_id, active_id);
        assert!(!session.gameplay.world().combatant(inactive_id).unwrap().active);

        let projection = session.mission5_content_projection();
        let projected_id = projection
            .shooter
            .as_ref()
            .expect("la proyección debe incluir un Shooter")
            .entity_id;
        assert_eq!(projected_id, active_id.value());
        assert_ne!(projected_id, inactive_id.value());
        assert_eq!(
            session
                .gameplay
                .world()
                .destination(crate::cooperacion::SIGNAL_DESTINATION)
                .unwrap()
                .id
                .value(),
            crate::cooperacion::SIGNAL_DESTINATION.value()
        );

        client.disconnect().unwrap();
    }

    #[test]
    fn proyeccion_mission5_omite_shooters_inactivos() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut client = GameClient::connect(address, "Jugador").unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut client),
            ServerMessage::Mission5ContentState { .. }
        ));

        let mission = crate::cooperacion::PROGRESION_COOPERATIVA;
        let setup = session.gameplay.update([
            GameplayCommand::UnlockMission { mission },
            GameplayCommand::StartMission { mission },
        ]);
        assert!(setup.iter().all(|result| matches!(
            result.status,
            CommandStatus::Success { .. }
        )));

        let active_id = session
            .gameplay
            .world()
            .combatants()
            .find(|combatant| {
                !combatant.is_player
                    && combatant.active
                    && combatant.archetype == Some(gameplay::EnemyArchetype::Shooter)
            })
            .map(|combatant| combatant.entity)
            .expect("la preparacion debe crear un Shooter activo");
        let before = session.gameplay.mission_progress(mission).unwrap();
        let destination_id = session
            .gameplay
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .unwrap()
            .id
            .value();

        let deactivate_result = session.gameplay.update([GameplayCommand::SetEnemyActive {
            enemy: active_id,
            active: false,
        }]);
        assert!(matches!(
            deactivate_result.first().map(|result| result.status),
            Some(CommandStatus::Success { changed: true })
        ));

        let projection = session.mission5_content_projection();
        assert!(projection.shooter.is_none());
        assert_eq!(
            projection
                .destination
                .as_ref()
                .expect("Destination104 debe conservarse")
                .destination_id,
            destination_id
        );
        let after = session.gameplay.mission_progress(mission).unwrap();
        assert_eq!(projection.required_completed, before.required_completed as u32);
        assert_eq!(projection.optional_completed, before.optional_completed as u32);
        assert_eq!(after.required_completed, before.required_completed);
        assert_eq!(after.optional_completed, before.optional_completed);

        client.disconnect().unwrap();
    }

    #[test]
    fn movimiento_invalido_no_muta_y_dos_clientes_reciben_el_movimiento_autorizado() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut first = GameClient::connect(address, "Jugador 1").unwrap();
        wait_for_update(&mut session);
        assert!(matches!(
            receive_non_bootstrap(&mut first),
            ServerMessage::Mission5ContentState { .. }
        ));
        let mut second = GameClient::connect(address, "Jugador 2").unwrap();
        wait_for_player(&mut session, second.player_id());
        let first_entity = session.network_player(first.player_id()).unwrap().entity;
        let before = session
            .gameplay()
            .world()
            .player(first_entity)
            .unwrap()
            .position;
        first
            .send(&ClientMessage::PlayerMove {
                sequence: 1,
                transform: PlayerTransform::new(
                    NetworkVec3::new(0.0, 0.0, 0.0),
                    NetworkOrientation::new(0.0, 0.0),
                    NetworkVec3::new(2.0, 0.0, 0.0),
                ),
            })
            .unwrap();
        wait_for_update(&mut session);
        assert_eq!(
            session
                .gameplay()
                .world()
                .player(first_entity)
                .unwrap()
                .position,
            before
        );

        first
            .send(&ClientMessage::PlayerMove {
                sequence: 2,
                transform: PlayerTransform::new(
                    NetworkVec3::new(999.0, 999.0, 999.0),
                    NetworkOrientation::new(15.0, 0.0),
                    NetworkVec3::new(1.0, 0.0, 0.0),
                ),
            })
            .unwrap();
        wait_for_update(&mut session);
        assert_ne!(
            session
                .gameplay()
                .world()
                .player(first_entity)
                .unwrap()
                .position,
            before
        );

        assert!(matches!(
            receive_non_bootstrap(&mut second),
            ServerMessage::PlayerJoined { .. }
        ));
        assert!(matches!(
            receive_non_bootstrap(&mut second),
            ServerMessage::Mission5ContentState { .. }
        ));
        assert!(
            matches!(receive_non_bootstrap(&mut second), ServerMessage::PlayerMoved { player_id, sequence: 2, .. } if player_id == first.player_id())
        );

        first.disconnect().unwrap();
        for _ in 0..100 {
            session.update().unwrap();
            if session.network_player(first.player_id()).is_none() {
                break;
            }
            thread::sleep(StdDuration::from_millis(5));
        }
        assert!(session.network_player(first.player_id()).is_none());
        second.disconnect().unwrap();
    }

    fn wait_until<F>(session: &mut MultiplayerSession, mut condition: F)
    where
        F: FnMut(&MultiplayerSession) -> bool,
    {
        let deadline = std::time::Instant::now() + StdDuration::from_secs(3);
        loop {
            session.update().unwrap();
            if condition(session) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Se agotó el tiempo esperando el estado multijugador"
            );
            thread::sleep(StdDuration::from_millis(5));
        }
    }

    fn send_test_movement(client: &mut GameClient, sequence: u64) {
        client
            .send(&ClientMessage::PlayerMove {
                sequence,
                transform: PlayerTransform::new(
                    NetworkVec3::new(0.0, 0.0, 0.0),
                    NetworkOrientation::new(0.0, 0.0),
                    NetworkVec3::new(1.0, 0.0, 0.0),
                ),
            })
            .unwrap();
    }

    #[test]
    fn tres_jugadores_tienen_entidades_distintas() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();

        let mut first = GameClient::connect(address, "Ruth").unwrap();
        wait_for_player(&mut session, first.player_id());
        let first_content = receive_initial_mission5_state(&mut first);
        let mut second = GameClient::connect(address, "Tiffany").unwrap();
        wait_for_player(&mut session, second.player_id());
        let second_content = receive_initial_mission5_state(&mut second);
        let mut third = GameClient::connect(address, "Sarita").unwrap();
        wait_for_player(&mut session, third.player_id());
        let third_content = receive_initial_mission5_state(&mut third);

        assert_eq!(first_content, session.mission5_content_projection());
        assert_eq!(second_content, first_content);
        assert_eq!(third_content, first_content);

        let entities = [first.player_id(), second.player_id(), third.player_id()]
            .map(|id| session.network_player(id).unwrap().entity);

        assert_eq!(entities[0], session.gameplay.ids.first_player);
        assert_ne!(entities[0], entities[1]);
        assert_ne!(entities[0], entities[2]);
        assert_ne!(entities[1], entities[2]);
        assert_eq!(session.gameplay().world().players().count(), 3);

        first.disconnect().unwrap();
        second.disconnect().unwrap();
        third.disconnect().unwrap();
    }

    #[test]
    fn desconexion_libera_entidad_para_nuevo_jugador() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();

        let mut first = GameClient::connect(address, "Ruth").unwrap();
        wait_for_player(&mut session, first.player_id());
        let mut second = GameClient::connect(address, "Tiffany").unwrap();
        wait_for_player(&mut session, second.player_id());
        let mut third = GameClient::connect(address, "Sarita").unwrap();
        wait_for_player(&mut session, third.player_id());

        let released_entity = session.network_player(second.player_id()).unwrap().entity;
        second.disconnect().unwrap();
        wait_until(&mut session, |s| {
            s.network_player(second.player_id()).is_none()
        });
        assert!(
            !session
                .gameplay()
                .world()
                .player(released_entity)
                .unwrap()
                .is_active
        );

        let mut replacement = GameClient::connect(address, "Jugador 4").unwrap();
        wait_for_player(&mut session, replacement.player_id());
        assert_eq!(
            session
                .network_player(replacement.player_id())
                .unwrap()
                .entity,
            released_entity
        );
        assert!(
            session
                .gameplay()
                .world()
                .player(released_entity)
                .unwrap()
                .is_active
        );
        assert_eq!(session.gameplay().world().players().count(), 3);

        first.disconnect().unwrap();
        third.disconnect().unwrap();
        replacement.disconnect().unwrap();
    }

    #[test]
    fn jugador_reconectado_puede_moverse() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();

        let mut first = GameClient::connect(address, "Ruth").unwrap();
        wait_for_player(&mut session, first.player_id());
        let mut second = GameClient::connect(address, "Tiffany").unwrap();
        wait_for_player(&mut session, second.player_id());
        let mut third = GameClient::connect(address, "Sarita").unwrap();
        wait_for_player(&mut session, third.player_id());

        let released_entity = session.network_player(first.player_id()).unwrap().entity;
        first.disconnect().unwrap();
        wait_until(&mut session, |s| {
            s.network_player(first.player_id()).is_none()
        });

        let mut replacement = GameClient::connect(address, "Jugador 4").unwrap();
        wait_for_player(&mut session, replacement.player_id());
        assert_eq!(
            session
                .network_player(replacement.player_id())
                .unwrap()
                .entity,
            released_entity
        );

        let initial_position = session
            .gameplay()
            .world()
            .player(released_entity)
            .unwrap()
            .position;
        send_test_movement(&mut replacement, 1);
        wait_until(&mut session, |s| {
            s.gameplay()
                .world()
                .player(released_entity)
                .is_some_and(|player| player.position != initial_position)
        });

        assert!(session.network_player(second.player_id()).is_some());
        assert!(session.network_player(third.player_id()).is_some());
        assert_eq!(session.gameplay().world().players().count(), 3);

        second.disconnect().unwrap();
        third.disconnect().unwrap();
        replacement.disconnect().unwrap();
    }
}
