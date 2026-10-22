use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use engine::{Duration, EntityId, Vec3};
use gameplay::{CommandStatus, GameplayCommand};
use networking::{
    GameServer, GameServerHandle, NetworkOrientation, NetworkPlayerId, NetworkVec3,
    PlayerTransform, ServerEvent, ServerEventQueue, ServerMessage,
};

use crate::ProjectEchoSession;

const NETWORK_TICK: Duration = Duration(100);

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
    shutdown: Arc<AtomicBool>,
    server_thread: Option<JoinHandle<io::Result<()>>>,
}

impl MultiplayerSession {
    pub fn bind(address: &str) -> Result<Self, MultiplayerError> {
        let server = GameServer::bind(address)?;
        let local_address = server.local_addr()?;
        let server_handle = server.handle();
        let events = server.event_queue();
        let shutdown = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&shutdown);
        let server_events = events.clone();
        let server_thread =
            thread::spawn(move || server.run_until_with_events(&signal, Some(server_events)));

        Ok(Self {
            gameplay: ProjectEchoSession::new(),
            server: server_handle,
            address: local_address,
            events,
            players: BTreeMap::new(),
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
        let reusable = self
            .gameplay
            .world()
            .players()
            .find(|player| !player.is_active)
            .map(|player| player.entity_id);
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
        } else {
            self.gameplay
                .world_mut()
                .create_player()
                .map_err(MultiplayerError::PlayerCreation)?
        };
        self.players.insert(player_id, entity);
        Ok(())
    }

    fn unregister_player(&mut self, player_id: NetworkPlayerId) {
        if let Some(entity) = self.players.remove(&player_id) {
            let _ = self.gameplay.update([GameplayCommand::SetPlayerActive {
                player: entity,
                active: false,
            }]);
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
        let updated = self
            .gameplay
            .world()
            .player(entity)
            .copied()
            .ok_or(MultiplayerError::UnknownPlayer(player_id))?;
        self.server.broadcast_except(
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
            Some(player_id),
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

    #[test]
    fn ingreso_crea_una_entidad_estable_y_movimiento_se_aplica() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut client = GameClient::connect(address, "Jugador").unwrap();
        wait_for_update(&mut session);
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
    fn movimiento_invalido_no_muta_y_dos_clientes_reciben_el_movimiento_autorizado() {
        let mut session = MultiplayerSession::bind("127.0.0.1:0").unwrap();
        let address = session.local_addr().unwrap();
        let mut first = GameClient::connect(address, "Jugador 1").unwrap();
        wait_for_update(&mut session);
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
            second.receive().unwrap(),
            ServerMessage::PlayerJoined { .. }
        ));
        assert!(
            matches!(second.receive().unwrap(), ServerMessage::PlayerMoved { player_id, sequence: 2, .. } if player_id == first.player_id())
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
}