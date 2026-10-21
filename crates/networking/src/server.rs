use crate::protocol::{
    decode_client, encode, ClientMessage, NetworkPlayerId, ServerMessage, MAX_PLAYERS,
    PROTOCOL_VERSION,
};

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Default)]
struct ServerState {
    next_id: NetworkPlayerId,
    players: BTreeMap<NetworkPlayerId, String>,
    connections: BTreeMap<NetworkPlayerId, TcpStream>,
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
        self.players.remove(&id).is_some()
    }

    fn count(&self) -> usize {
        self.players.len()
    }

    fn broadcast(&mut self, message: &ServerMessage, exclude: Option<NetworkPlayerId>) {
        let mut failed = Vec::new();

        for (&id, stream) in &mut self.connections {
            if Some(id) == exclude {
                continue;
            }

            if send_message(stream, message).is_err() {
                failed.push(id);
            }
        }

        for id in failed {
            self.connections.remove(&id);
        }
    }
}

pub struct GameServer {
    listener: TcpListener,
    state: Arc<Mutex<ServerState>>,
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

    pub fn run_until(&self, shutdown: &AtomicBool) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;

        while !shutdown.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let state = Arc::clone(&self.state);

                    thread::spawn(move || {
                        if let Err(error) = handle_client(stream, state) {
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

        Ok(())
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
                        if let Err(error) = handle_client(stream, state) {
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

fn handle_client(mut stream: TcpStream, state: Arc<Mutex<ServerState>>) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.set_nodelay(true)?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut player_id: Option<NetworkPlayerId> = None;

    let result = (|| -> io::Result<()> {
        loop {
            let mut line = Vec::new();

            let bytes = reader
                .by_ref()
                .take((MAX_MESSAGE_SIZE + 1) as u64)
                .read_until(b'\n', &mut line)?;

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

                    send_message(
                        &mut stream,
                        &ServerMessage::Welcome {
                            player_id: id,
                            max_players: MAX_PLAYERS,
                        },
                    )?;

                    let existing = server
                        .players
                        .iter()
                        .filter(|(other_id, _)| **other_id != id)
                        .map(|(&other_id, other_name)| (other_id, other_name.clone()))
                        .collect::<Vec<_>>();

                    for (other_id, other_name) in existing {
                        send_message(
                            &mut stream,
                            &ServerMessage::PlayerJoined {
                                player_id: other_id,
                                name: other_name,
                            },
                        )?;
                    }

                    server.connections.insert(id, stream.try_clone()?);

                    server.broadcast(
                        &ServerMessage::PlayerJoined {
                            player_id: id,
                            name: name.trim().to_string(),
                        },
                        Some(id),
                    );

                    println!("Jugador {id} conectado: {name}");
                }

                ClientMessage::Leave => {
                    break;
                }

                ClientMessage::PlayerMove {
                    sequence: _,
                    transform: _,
                } => {
                    // La recepción, validación y difusión
                    // de movimientos se implementará
                    // en la siguiente fase.
                }

                ClientMessage::Ping => {
                    if let Some(id) = player_id {
                        let mut server = state
                            .lock()
                            .map_err(|_| io::Error::other("Estado del servidor no disponible"))?;

                        if let Some(connection) = server.connections.get_mut(&id) {
                            send_message(connection, &ServerMessage::Pong)?;
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
        let mut server = match state.lock() {
            Ok(server) => server,
            Err(poisoned) => poisoned.into_inner(),
        };

        if server.leave(id) {
            server.broadcast(&ServerMessage::PlayerLeft { player_id: id }, Some(id));

            println!(
                "Jugador {id} desconectado. Jugadores activos: {}",
                server.count()
            );
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
