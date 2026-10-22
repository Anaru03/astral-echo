use crate::protocol::{
    decode_client, encode, ClientMessage, NetworkPlayerId, ServerMessage, MAX_PLAYERS,
    PROTOCOL_VERSION,
};
use crate::sync::PlayerTransform;
use crate::{ServerEvent, ServerEventQueue};

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Default)]
struct ServerState {
    next_id: NetworkPlayerId,
    players: BTreeMap<NetworkPlayerId, String>,
    connections: BTreeMap<NetworkPlayerId, Connection>,
    last_sequences: BTreeMap<NetworkPlayerId, u64>,
}

#[derive(Debug)]
struct Connection {
    stream: TcpStream,
    write_lock: Arc<Mutex<()>>,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            write_lock: Arc::new(Mutex::new(())),
        }
    }
}

impl ServerState {
    fn join(&mut self, name: String) -> Result<NetworkPlayerId, String> {
        let name = name.trim();

        if name.is_empty() || name.len() > 32 {
            return Err("Nombre de jugador inválido".to_string());
        }

        if self.players.len() >= MAX_PLAYERS {
            return Err("La partida está llena".to_string());
        }

        let id = self
            .next_id
            .checked_add(1)
            .ok_or("No hay identificadores disponibles")?;

        self.next_id = id;
        self.players.insert(id, name.to_string());

        Ok(id)
    }

    fn leave(&mut self, id: NetworkPlayerId) -> bool {
        self.connections.remove(&id);
        self.last_sequences.remove(&id);
        self.players.remove(&id).is_some()
    }

    fn count(&self) -> usize {
        self.players.len()
    }

    fn accept_movement(
        &mut self,
        player_id: NetworkPlayerId,
        sequence: u64,
        transform: PlayerTransform,
    ) -> bool {
        if !self.players.contains_key(&player_id) || !transform.is_valid() {
            return false;
        }

        if let Some(&previous) = self.last_sequences.get(&player_id) {
            if sequence <= previous {
                return false;
            }
        }

        self.last_sequences.insert(player_id, sequence);
        true
    }

}

fn broadcast_state(
    state: &Arc<Mutex<ServerState>>,
    message: &ServerMessage,
    exclude: Option<NetworkPlayerId>,
) -> Vec<NetworkPlayerId> {
    let targets = match state.lock() {
        Ok(server) => server
            .connections
            .iter()
            .filter(|(id, _)| Some(**id) != exclude)
            .filter_map(|(&id, connection)| {
                connection
                    .stream
                    .try_clone()
                    .ok()
                    .map(|stream| (id, stream, Arc::clone(&connection.write_lock)))
            })
            .collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };

    let failed = targets
        .into_iter()
        .filter_map(|(id, mut stream, write_lock)| {
            let _guard = write_lock.lock().ok()?;
            send_message(&mut stream, message).err().map(|_| id)
        })
        .collect::<Vec<_>>();

    let mut disconnected = Vec::new();
    if let Ok(mut server) = state.lock() {
        for id in failed {
            if server.leave(id) {
                disconnected.push(id);
            }
        }
    }
    disconnected
}

fn publish_disconnections(events: Option<&ServerEventQueue>, player_ids: Vec<NetworkPlayerId>) {
    if let Some(events) = events {
        for player_id in player_ids {
            events.push(ServerEvent::PlayerLeft { player_id });
        }
    }
}

fn disconnect_player(
    state: &Arc<Mutex<ServerState>>,
    events: Option<&ServerEventQueue>,
    player_id: NetworkPlayerId,
) -> bool {
    let removed = state
        .lock()
        .map(|mut server| server.leave(player_id))
        .unwrap_or(false);
    if !removed {
        return false;
    }

    let disconnected = broadcast_state(
        state,
        &ServerMessage::PlayerLeft { player_id },
        Some(player_id),
    );
    publish_disconnections(events, disconnected);
    if let Some(events) = events {
        events.push(ServerEvent::PlayerLeft { player_id });
    }
    true
}

pub struct GameServer {
    listener: TcpListener,
    state: Arc<Mutex<ServerState>>,
}

#[derive(Default)]
struct ActiveConnections {
    closing: AtomicBool,
    next_id: AtomicU64,
    streams: Mutex<BTreeMap<u64, TcpStream>>,
}

impl ActiveConnections {
    fn register(&self, stream: &TcpStream) -> io::Result<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let stream = stream.try_clone()?;
        let mut streams = self
            .streams
            .lock()
            .map_err(|_| io::Error::other("Registro de conexiones no disponible"))?;
        streams.insert(id, stream);
        Ok(id)
    }

    fn remove(&self, id: u64) {
        if let Ok(mut streams) = self.streams.lock() {
            streams.remove(&id);
        }
    }

    fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }

    fn shutdown_all(&self) {
        self.closing.store(true, Ordering::Release);
        let streams = self
            .streams
            .lock()
            .map(|mut streams| {
                std::mem::take(&mut *streams).into_values().collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for stream in streams {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

#[derive(Clone)]
pub struct GameServerHandle {
    state: Arc<Mutex<ServerState>>,
    events: Option<ServerEventQueue>,
}

impl GameServerHandle {
    pub fn broadcast(&self, message: &ServerMessage) {
        self.broadcast_except(message, None);
    }

    pub fn broadcast_except(
        &self,
        message: &ServerMessage,
        exclude: Option<NetworkPlayerId>,
    ) {
        let disconnected = broadcast_state(&self.state, message, exclude);
        publish_disconnections(self.events.as_ref(), disconnected);
    }
}

impl GameServer {
    pub fn bind(address: &str) -> io::Result<Self> {
        let listener = TcpListener::bind(address)?;

        Ok(Self {
            listener,
            state: Arc::new(Mutex::new(ServerState::default())),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn handle(&self) -> GameServerHandle {
        GameServerHandle {
            state: Arc::clone(&self.state),
            events: None,
        }
    }

    pub fn handle_with_events(&self, events: ServerEventQueue) -> GameServerHandle {
        GameServerHandle {
            state: Arc::clone(&self.state),
            events: Some(events),
        }
    }

    pub fn event_queue(&self) -> ServerEventQueue {
        ServerEventQueue::new()
    }

    pub fn run_until(&self, shutdown: &AtomicBool) -> io::Result<()> {
        self.run_until_with_events(shutdown, None)
    }

    pub fn run_until_with_events(
        &self,
        shutdown: &AtomicBool,
        events: Option<ServerEventQueue>,
    ) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;
        let active_connections = Arc::new(ActiveConnections::default());
        let mut client_threads: Vec<JoinHandle<()>> = Vec::new();
        let mut server_error = None;

        while !shutdown.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let connection_id = match active_connections.register(&stream) {
                        Ok(connection_id) => connection_id,
                        Err(error) => {
                            server_error = Some(error);
                            break;
                        }
                    };
                    let state = Arc::clone(&self.state);
                    let events = events.clone();
                    let active_connections = Arc::clone(&active_connections);

                    client_threads.push(thread::spawn(move || {
                        if let Err(error) = handle_client(
                            stream,
                            state,
                            events,
                            Some(Arc::clone(&active_connections)),
                        ) {
                            eprintln!("Error de conexión: {error}");
                        }
                        active_connections.remove(connection_id);
                    }));
                }

                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }

                Err(_error) if shutdown.load(Ordering::Relaxed) => break,
                Err(error) => {
                    server_error = Some(error);
                    break;
                }
            }
        }

        active_connections.shutdown_all();
        for client_thread in client_threads {
            client_thread
                .join()
                .map_err(|_| io::Error::other("El hilo de cliente terminó inesperadamente"))?;
        }

        server_error.map_or(Ok(()), Err)
    }

    pub fn run(&self) -> io::Result<()> {
        println!("Servidor Astral Echo iniciado");
        println!("Escuchando en {}", self.local_addr()?);
        println!("Máximo de jugadores: {}", MAX_PLAYERS);

        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let state = Arc::clone(&self.state);

                    thread::spawn(move || {
                        if let Err(error) = handle_client(stream, state, None, None) {
                            eprintln!("Error de conexión: {error}");
                        }
                    });
                }

                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }

                Err(error) => return Err(error),
            }
        }
    }
}

fn send_message(stream: &mut TcpStream, message: &ServerMessage) -> io::Result<()> {
    let json =
        encode(message).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    stream.write_all(json.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn handle_client(
    mut stream: TcpStream,
    state: Arc<Mutex<ServerState>>,
    events: Option<ServerEventQueue>,
    active_connections: Option<Arc<ActiveConnections>>,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    let read_timeout = if active_connections.is_some() {
        Duration::from_millis(100)
    } else {
        Duration::from_secs(30)
    };
    stream.set_read_timeout(Some(read_timeout))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.set_nodelay(true)?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut player_id: Option<NetworkPlayerId> = None;

    let result = (|| -> io::Result<()> {
        loop {
            let mut line = Vec::new();

            let bytes = match reader
                .by_ref()
                .take((MAX_MESSAGE_SIZE + 1) as u64)
                .read_until(b'\n', &mut line)
            {
                Ok(bytes) => bytes,
                Err(error)
                    if error.kind() == io::ErrorKind::TimedOut
                        && active_connections
                            .as_ref()
                            .is_some_and(|connections| !connections.is_closing()) =>
                {
                    continue;
                }
                Err(error)
                    if error.kind() == io::ErrorKind::TimedOut
                        && active_connections
                            .as_ref()
                            .is_some_and(|connections| connections.is_closing()) =>
                {
                    break;
                }
                Err(error) => return Err(error),
            };

            if bytes == 0 {
                break;
            }

            if bytes > MAX_MESSAGE_SIZE || !line.ends_with(b"\n") {
                send_message(
                    &mut stream,
                    &ServerMessage::Rejected {
                        reason: "Mensaje demasiado largo o incompleto".to_string(),
                    },
                )?;
                break;
            }

            let text = match std::str::from_utf8(&line) {
                Ok(text) => text,
                Err(_) => {
                    send_message(
                        &mut stream,
                        &ServerMessage::Rejected {
                            reason: "Codificación UTF-8 inválida".to_string(),
                        },
                    )?;
                    break;
                }
            };

            let message = match decode_client(text.trim_end()) {
                Ok(message) => message,
                Err(_) => {
                    send_message(
                        &mut stream,
                        &ServerMessage::Rejected {
                            reason: "Mensaje JSON inválido".to_string(),
                        },
                    )?;
                    break;
                }
            };

            match message {
                ClientMessage::Join { version, name } => {
                    if player_id.is_some() {
                        send_message(
                            &mut stream,
                            &ServerMessage::Rejected {
                                reason: "Ya estás conectado".to_string(),
                            },
                        )?;
                        continue;
                    }

                    if version != PROTOCOL_VERSION {
                        send_message(
                            &mut stream,
                            &ServerMessage::Rejected {
                                reason: "Versión de protocolo incompatible".to_string(),
                            },
                        )?;
                        break;
                    }

                    let mut server = state
                        .lock()
                        .map_err(|_| io::Error::other("Estado del servidor no disponible"))?;

                    let id = match server.join(name.clone()) {
                        Ok(id) => id,
                        Err(reason) => {
                            drop(server);

                            send_message(&mut stream, &ServerMessage::Rejected { reason })?;

                            break;
                        }
                    };

                    player_id = Some(id);
                    let existing = server
                        .players
                        .iter()
                        .filter(|(other_id, _)| **other_id != id)
                        .map(|(&other_id, other_name)| (other_id, other_name.clone()))
                        .collect::<Vec<_>>();

                    let connection = Connection::new(stream.try_clone()?);
                    let write_lock = Arc::clone(&connection.write_lock);
                    server.connections.insert(id, connection);
                    drop(server);

                    let _guard = write_lock
                        .lock()
                        .map_err(|_| io::Error::other("Lock de escritura no disponible"))?;
                    send_message(
                        &mut stream,
                        &ServerMessage::Welcome {
                            player_id: id,
                            max_players: MAX_PLAYERS,
                        },
                    )?;
                    for (other_id, other_name) in existing {
                        send_message(
                            &mut stream,
                            &ServerMessage::PlayerJoined {
                                player_id: other_id,
                                name: other_name,
                            },
                        )?;
                    }
                    drop(_guard);

                    let disconnected = broadcast_state(
                        &state,
                        &ServerMessage::PlayerJoined {
                            player_id: id,
                            name: name.trim().to_string(),
                        },
                        Some(id),
                    );
                    publish_disconnections(events.as_ref(), disconnected);

                    if let Some(events) = &events {
                        events.push(ServerEvent::PlayerJoined {
                            player_id: id,
                            name: name.trim().to_string(),
                        });
                    }

                    println!("Jugador {id} conectado: {name}");
                }

                ClientMessage::Leave => {
                    break;
                }

                ClientMessage::PlayerMove {
                    sequence,
                    transform,
                } => {
                    if let Some(id) = player_id {
                        let accepted = state
                            .lock()
                            .map_err(|_| io::Error::other("Estado del servidor no disponible"))?
                            .accept_movement(id, sequence, transform);

                        if accepted {
                            let message = ServerMessage::PlayerMoved {
                                player_id: id,
                                sequence,
                                transform,
                            };
                            if events.is_some() {
                                if let Some(events) = &events {
                                    events.push(ServerEvent::PlayerMoved {
                                        player_id: id,
                                        sequence,
                                        transform,
                                    });
                                }
                            } else {
                                let disconnected = broadcast_state(&state, &message, Some(id));
                                publish_disconnections(events.as_ref(), disconnected);
                            }
                        }
                    }
                }

                ClientMessage::Ping => {
                    if let Some(id) = player_id {
                        let connection = state
                            .lock()
                            .map_err(|_| io::Error::other("Estado del servidor no disponible"))?
                            .connections
                            .get(&id)
                            .and_then(|connection| {
                                connection
                                    .stream
                                    .try_clone()
                                    .ok()
                                    .map(|stream| (stream, Arc::clone(&connection.write_lock)))
                            });

                        if let Some((mut connection, write_lock)) = connection {
                            let _guard = write_lock
                                .lock()
                                .map_err(|_| io::Error::other("Lock de escritura no disponible"))?;
                            if let Err(error) = send_message(&mut connection, &ServerMessage::Pong)
                            {
                                disconnect_player(&state, events.as_ref(), id);
                                return Err(error);
                            }
                        } else {
                            send_message(&mut stream, &ServerMessage::Pong)?;
                        }
                    } else {
                        send_message(&mut stream, &ServerMessage::Pong)?;
                    }
                }
            }
        }

        Ok(())
    })();

    if let Some(id) = player_id {
        if disconnect_player(&state, events.as_ref(), id) {
            let count = state.lock().map(|server| server.count()).unwrap_or(0);
            println!("Jugador {id} desconectado. Jugadores activos: {count}");
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{NetworkOrientation, NetworkVec3};

    fn sample_transform() -> PlayerTransform {
        PlayerTransform::new(
            NetworkVec3::new(1.0, 2.0, 3.0),
            NetworkOrientation::new(90.0, 0.0),
            NetworkVec3::new(0.0, 0.0, 1.0),
        )
    }

    #[test]
    fn test_join_player() {
        let mut state = ServerState::default();

        let id = state.join("Jugador 1".to_string()).unwrap();

        assert_eq!(id, 1);
        assert_eq!(state.count(), 1);
    }

    #[test]
    fn test_max_players() {
        let mut state = ServerState::default();

        for i in 0..MAX_PLAYERS {
            assert!(state.join(format!("Jugador {}", i + 1)).is_ok());
        }

        assert!(state.join("Jugador 4".to_string()).is_err());
        assert_eq!(state.count(), MAX_PLAYERS);
    }

    #[test]
    fn test_player_leave() {
        let mut state = ServerState::default();

        let id = state.join("Jugador 1".to_string()).unwrap();

        assert!(state.leave(id));
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn test_invalid_name() {
        let mut state = ServerState::default();

        assert!(state.join("".to_string()).is_err());
        assert!(state.join("   ".to_string()).is_err());
    }

    #[test]
    fn test_unique_player_ids() {
        let mut state = ServerState::default();

        let first = state.join("Jugador 1".to_string()).unwrap();
        state.leave(first);

        let second = state.join("Jugador 2".to_string()).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn test_accept_valid_movement() {
        let mut state = ServerState::default();
        let id = state.join("Jugador 1".to_string()).unwrap();

        assert!(state.accept_movement(id, 1, sample_transform()));
    }

    #[test]
    fn test_reject_old_movement() {
        let mut state = ServerState::default();
        let id = state.join("Jugador 1".to_string()).unwrap();

        let transform = sample_transform();

        assert!(state.accept_movement(id, 5, transform));
        assert!(!state.accept_movement(id, 3, transform));
        assert!(!state.accept_movement(id, 5, transform));
        assert!(state.accept_movement(id, 6, transform));
    }

    #[test]
    fn test_reject_invalid_movement() {
        let mut state = ServerState::default();
        let id = state.join("Jugador 1".to_string()).unwrap();

        let mut transform = sample_transform();
        transform.position.x = f32::NAN;

        assert!(!state.accept_movement(id, 1, transform));
    }

    #[test]
    fn test_reject_unknown_player_movement() {
        let mut state = ServerState::default();

        assert!(!state.accept_movement(999, 1, sample_transform()));
    }

    #[test]
    fn test_clear_sequence_on_leave() {
        let mut state = ServerState::default();
        let id = state.join("Jugador 1".to_string()).unwrap();

        assert!(state.accept_movement(id, 10, sample_transform()));
        assert!(state.last_sequences.contains_key(&id));

        state.leave(id);

        assert!(!state.last_sequences.contains_key(&id));
    }

    #[test]
    fn test_broadcast_failure_cleans_player_once_and_preserves_healthy_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let healthy_client = TcpStream::connect(address).unwrap();
        let (healthy_server, _) = listener.accept().unwrap();
        let failed_client = TcpStream::connect(address).unwrap();
        let (failed_server, _) = listener.accept().unwrap();
        failed_server.shutdown(Shutdown::Both).unwrap();

        let mut initial = ServerState::default();
        let healthy_id = initial.join("Jugador sano".to_string()).unwrap();
        let failed_id = initial.join("Jugador fallido".to_string()).unwrap();
        initial
            .connections
            .insert(healthy_id, Connection::new(healthy_server));
        initial
            .connections
            .insert(failed_id, Connection::new(failed_server));
        let state = Arc::new(Mutex::new(initial));
        let events = ServerEventQueue::new();
        let handle = GameServerHandle {
            state: Arc::clone(&state),
            events: Some(events.clone()),
        };

        handle.broadcast(&ServerMessage::PlayerJoined {
            player_id: healthy_id,
            name: "Jugador sano".to_string(),
        });

        let mut reader = BufReader::new(healthy_client);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            crate::decode_server(line.trim()).unwrap(),
            ServerMessage::PlayerJoined {
                player_id: healthy_id,
                name: "Jugador sano".to_string(),
            }
        );

        let server = state.lock().unwrap();
        assert!(server.players.contains_key(&healthy_id));
        assert!(!server.players.contains_key(&failed_id));
        assert!(!server.connections.contains_key(&failed_id));
        drop(server);

        assert_eq!(
            events.pop(),
            Some(ServerEvent::PlayerLeft {
                player_id: failed_id,
            })
        );
        assert_eq!(events.pop(), None);
        assert!(!disconnect_player(&state, Some(&events), failed_id));
        drop(failed_client);
    }
}