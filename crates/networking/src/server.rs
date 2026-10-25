use crate::protocol::{
    decode_client, encode, ClientMessage, NetworkPlayerId, ServerMessage, MAX_PLAYERS,
    PROTOCOL_VERSION,
};
use crate::sync::PlayerTransform;
use crate::{ServerEvent, ServerEventQueue};

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_MESSAGE_SIZE: usize = 4096;
const MAX_PENDING_OUTPUT_MESSAGES: usize = 32;
const MAX_PENDING_CONNECTIONS: usize = 3;
const JOIN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct ServerState {
    next_id: NetworkPlayerId,
    players: BTreeMap<NetworkPlayerId, String>,
    connections: BTreeMap<NetworkPlayerId, Connection>,
    last_sequences: BTreeMap<NetworkPlayerId, u64>,
    publication: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct Connection {
    stream: TcpStream,
    output: Option<Arc<OutputQueue>>,
    legacy_write_lock: Arc<Mutex<()>>,
}

impl Connection {
    fn new(stream: TcpStream, output: Option<Arc<OutputQueue>>) -> Self {
        Self {
            stream,
            output,
            legacy_write_lock: Arc::new(Mutex::new(())),
        }
    }

    fn close(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
        if let Some(output) = &self.output {
            output.close();
        }
    }
}

#[derive(Debug)]
struct OutputState {
    queue: VecDeque<Vec<u8>>,
    closing: bool,
}

#[derive(Debug)]
struct OutputQueue {
    state: Mutex<OutputState>,
    wake: Condvar,
}

impl OutputQueue {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(OutputState {
                queue: VecDeque::new(),
                closing: false,
            }),
            wake: Condvar::new(),
        })
    }

    fn enqueue(&self, message: &ServerMessage) -> io::Result<()> {
        let json =
            encode(message).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut bytes = json.into_bytes();
        bytes.push(b'\n');
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("Cola de salida no disponible"))?;
        if state.closing {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Conexión cerrada",
            ));
        }
        if state.queue.len() >= MAX_PENDING_OUTPUT_MESSAGES {
            state.closing = true;
            state.queue.clear();
            self.wake.notify_all();
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Cola de salida llena",
            ));
        }
        state.queue.push_back(bytes);
        self.wake.notify_one();
        Ok(())
    }

    fn next(&self) -> Option<Vec<u8>> {
        let mut state = self.state.lock().ok()?;
        loop {
            if let Some(message) = state.queue.pop_front() {
                return Some(message);
            }
            if state.closing {
                return None;
            }
            state = self.wake.wait(state).ok()?;
        }
    }

    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closing = true;
            state.queue.clear();
            self.wake.notify_all();
        }
    }

    fn finish(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closing = true;
            self.wake.notify_all();
        }
    }
}

fn run_writer(mut stream: TcpStream, output: Arc<OutputQueue>) {
    while let Some(message) = output.next() {
        if stream
            .write_all(&message)
            .and_then(|_| stream.flush())
            .is_err()
        {
            output.close();
            let _ = stream.shutdown(Shutdown::Both);
            break;
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
        if let Some(connection) = self.connections.remove(&id) {
            connection.close();
        }
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

fn broadcast_state_inner(
    state: &Arc<Mutex<ServerState>>,
    message: &ServerMessage,
    exclude: Option<NetworkPlayerId>,
) -> Vec<NetworkPlayerId> {
    let mut server = match state.lock() {
        Ok(server) => server,
        Err(_) => return Vec::new(),
    };
    if let ServerMessage::PlayerMoved { player_id, .. } = message {
        if !server.players.contains_key(player_id) {
            return Vec::new();
        }
    }
    let mut failed = Vec::new();
    let targets = server
        .connections
        .iter()
        .filter(|(id, _)| Some(**id) != exclude)
        .filter_map(|(&id, connection)| {
            connection.stream.try_clone().ok().map(|stream| {
                (
                    id,
                    stream,
                    connection.output.clone(),
                    Arc::clone(&connection.legacy_write_lock),
                )
            })
        })
        .collect::<Vec<_>>();
    for (id, mut stream, output, legacy_write_lock) in targets {
        let result = if let Some(output) = output {
            output.enqueue(message)
        } else {
            let _guard = match legacy_write_lock.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    failed.push(id);
                    continue;
                }
            };
            send_message(&mut stream, message)
        };
        if result.is_err() {
            failed.push(id);
        }
    }
    let mut disconnected = Vec::new();
    for id in failed {
        if server.leave(id) {
            disconnected.push(id);
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

fn broadcast_disconnections_inner(
    state: &Arc<Mutex<ServerState>>,
    events: Option<&ServerEventQueue>,
    player_ids: Vec<NetworkPlayerId>,
) {
    let mut pending = player_ids;
    let mut index = 0;

    while index < pending.len() {
        let player_id = pending[index];
        index += 1;
        let failed = broadcast_state_inner(
            state,
            &ServerMessage::PlayerLeft { player_id },
            Some(player_id),
        );
        pending.extend(failed);
        publish_disconnections(events, vec![player_id]);
    }
}

fn broadcast_all(
    state: &Arc<Mutex<ServerState>>,
    events: Option<&ServerEventQueue>,
    message: &ServerMessage,
    exclude: Option<NetworkPlayerId>,
) {
    let publication = match state.lock() {
        Ok(server) => Arc::clone(&server.publication),
        Err(_) => return,
    };
    let _publication = match publication.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    broadcast_message_locked(state, events, message, exclude);
}

fn broadcast_message_locked(
    state: &Arc<Mutex<ServerState>>,
    events: Option<&ServerEventQueue>,
    message: &ServerMessage,
    exclude: Option<NetworkPlayerId>,
) {
    let disconnected = broadcast_state_inner(state, message, exclude);
    broadcast_disconnections_inner(state, events, disconnected);
}

fn disconnect_player(
    state: &Arc<Mutex<ServerState>>,
    events: Option<&ServerEventQueue>,
    player_id: NetworkPlayerId,
) -> bool {
    let publication = match state.lock() {
        Ok(server) => Arc::clone(&server.publication),
        Err(_) => return false,
    };
    let _publication = match publication.lock() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    let removed = state
        .lock()
        .map(|mut server| server.leave(player_id))
        .unwrap_or(false);
    if !removed {
        return false;
    }

    broadcast_disconnections_inner(state, events, vec![player_id]);
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
    pending: AtomicUsize,
    lifecycle: Mutex<()>,
    streams: Mutex<BTreeMap<u64, TcpStream>>,
}

impl ActiveConnections {
    fn reserve_pending(self: &Arc<Self>) -> Option<PendingRegistration> {
        let _lifecycle = self.lifecycle.lock().ok()?;
        if self.is_closing() {
            return None;
        }

        let mut current = self.pending.load(Ordering::Acquire);
        loop {
            if current >= MAX_PENDING_CONNECTIONS {
                return None;
            }
            match self.pending.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(PendingRegistration {
                        active_connections: Arc::clone(self),
                        released: false,
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn release_pending(&self) {
        let previous = self.pending.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0);
    }

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
        let _lifecycle = self.lifecycle.lock().ok();
        self.closing.store(true, Ordering::Release);
        let streams = self
            .streams
            .lock()
            .map(|mut streams| {
                std::mem::take(&mut *streams)
                    .into_values()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for stream in streams {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

struct PendingRegistration {
    active_connections: Arc<ActiveConnections>,
    released: bool,
}

impl PendingRegistration {
    fn release(&mut self) {
        if !self.released {
            self.active_connections.release_pending();
            self.released = true;
        }
    }
}

impl Drop for PendingRegistration {
    fn drop(&mut self) {
        self.release();
    }
}

fn reap_finished_client_threads(
    client_threads: &mut Vec<JoinHandle<()>>,
    server_error: &mut Option<io::Error>,
) {
    let mut index = 0;
    while index < client_threads.len() {
        if client_threads[index].is_finished() {
            let client_thread = client_threads.swap_remove(index);
            if client_thread.join().is_err() && server_error.is_none() {
                *server_error = Some(io::Error::other(
                    "El hilo de cliente terminó inesperadamente",
                ));
            }
        } else {
            index += 1;
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

    pub fn broadcast_except(&self, message: &ServerMessage, exclude: Option<NetworkPlayerId>) {
        broadcast_all(&self.state, self.events.as_ref(), message, exclude);
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
        self.run_until_with_events_timeout(shutdown, events, JOIN_TIMEOUT)
    }

    #[cfg(test)]
    fn run_until_with_events_for_test(
        &self,
        shutdown: &AtomicBool,
        events: Option<ServerEventQueue>,
        join_timeout: Duration,
    ) -> io::Result<()> {
        self.run_until_with_events_timeout(shutdown, events, join_timeout)
    }

    fn run_until_with_events_timeout(
        &self,
        shutdown: &AtomicBool,
        events: Option<ServerEventQueue>,
        join_timeout: Duration,
    ) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;
        let active_connections = Arc::new(ActiveConnections::default());
        let mut client_threads: Vec<JoinHandle<()>> = Vec::new();
        let mut server_error = None;

        while !shutdown.load(Ordering::Relaxed) {
            reap_finished_client_threads(&mut client_threads, &mut server_error);
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if shutdown.load(Ordering::Acquire) || active_connections.is_closing() {
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    }

                    let pending = match active_connections.reserve_pending() {
                        Some(pending) => pending,
                        None => {
                            let _ = stream.shutdown(Shutdown::Both);
                            continue;
                        }
                    };

                    let connection_id = match active_connections.register(&stream) {
                        Ok(connection_id) => connection_id,
                        Err(error) => {
                            drop(pending);
                            let _ = stream.shutdown(Shutdown::Both);
                            if server_error.is_none() {
                                server_error = Some(error);
                            }
                            continue;
                        }
                    };
                    let state = Arc::clone(&self.state);
                    let events = events.clone();
                    let active_for_handler = Arc::clone(&active_connections);
                    let accepted_at = Instant::now();

                    let thread = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        thread::spawn(move || {
                            if let Err(error) = handle_client(
                                stream,
                                state,
                                events,
                                Some(Arc::clone(&active_for_handler)),
                                Some(pending),
                                accepted_at,
                                join_timeout,
                            ) {
                                eprintln!("Error de conexión: {error}");
                            }
                            active_for_handler.remove(connection_id);
                        })
                    }));

                    match thread {
                        Ok(client_thread) => client_threads.push(client_thread),
                        Err(_) => {
                            active_connections.remove(connection_id);
                            if server_error.is_none() {
                                server_error =
                                    Some(io::Error::other("No se pudo crear el hilo de cliente"));
                            }
                        }
                    }
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
        let mut join_error = None;
        for client_thread in client_threads {
            if client_thread.join().is_err() && join_error.is_none() {
                join_error = Some(io::Error::other(
                    "El hilo de cliente terminó inesperadamente",
                ));
            }
        }

        server_error.or(join_error).map_or(Ok(()), Err)
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
                        if let Err(error) = handle_client(
                            stream,
                            state,
                            None,
                            None,
                            None,
                            Instant::now(),
                            JOIN_TIMEOUT,
                        ) {
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

fn send_response(
    output: Option<&Arc<OutputQueue>>,
    stream: &mut TcpStream,
    message: &ServerMessage,
) -> io::Result<()> {
    if let Some(output) = output {
        output.enqueue(message)
    } else {
        send_message(stream, message)
    }
}

#[cfg(test)]
struct ControlledWrite {
    started: std::sync::mpsc::Sender<()>,
    cancel: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
impl ControlledWrite {
    fn run(self) -> io::Result<()> {
        self.started
            .send(())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "test start signal closed"))?;
        self.cancel
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| match error {
                std::sync::mpsc::RecvTimeoutError::Timeout => {
                    io::Error::new(io::ErrorKind::TimedOut, "test write cancellation timed out")
                }
                std::sync::mpsc::RecvTimeoutError::Disconnected => {
                    io::Error::new(io::ErrorKind::Interrupted, "test write cancelled")
                }
            })?;
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "test write cancelled",
        ))
    }
}

fn handle_client(
    mut stream: TcpStream,
    state: Arc<Mutex<ServerState>>,
    events: Option<ServerEventQueue>,
    active_connections: Option<Arc<ActiveConnections>>,
    mut pending_registration: Option<PendingRegistration>,
    accepted_at: Instant,
    join_timeout: Duration,
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

    let output = active_connections.as_ref().map(|_| OutputQueue::new());
    let writer = if let Some(output) = output.as_ref() {
        let writer_stream = stream.try_clone()?;
        let writer_output = Arc::clone(output);
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            thread::spawn(move || run_writer(writer_stream, writer_output))
        })) {
            Ok(writer) => Some(writer),
            Err(_) => {
                let _ = stream.shutdown(Shutdown::Both);
                return Err(io::Error::other("No se pudo crear el escritor de salida"));
            }
        }
    } else {
        None
    };

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut player_id: Option<NetworkPlayerId> = None;

    let result = (|| -> io::Result<()> {
        let mut line = Vec::new();
        loop {
            if player_id.is_none()
                && active_connections.is_some()
                && accepted_at.elapsed() >= join_timeout
            {
                break;
            }

            let mut complete = false;
            let mut ended = false;
            let mut stop_without_response = false;
            loop {
                if player_id.is_none()
                    && active_connections.is_some()
                    && accepted_at.elapsed() >= join_timeout
                {
                    ended = true;
                    stop_without_response = true;
                    break;
                }

                let available = match reader.fill_buf() {
                    Ok(available) if available.is_empty() => {
                        ended = true;
                        break;
                    }
                    Ok(available) => available,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        ) =>
                    {
                        if active_connections
                            .as_ref()
                            .is_some_and(|connections| connections.is_closing())
                            || (player_id.is_none()
                                && active_connections.is_some()
                                && accepted_at.elapsed() >= join_timeout)
                        {
                            ended = true;
                            stop_without_response = true;
                            break;
                        }
                        continue;
                    }
                    Err(error) => return Err(error),
                };

                let newline = available.iter().position(|byte| *byte == b'\n');
                let count = newline.map_or(available.len(), |index| index + 1);
                if line.len() + count > MAX_MESSAGE_SIZE {
                    line.clear();
                    send_response(
                        output.as_ref(),
                        &mut stream,
                        &ServerMessage::Rejected {
                            reason: "Mensaje demasiado largo o incompleto".to_string(),
                        },
                    )?;
                    ended = true;
                    break;
                }

                line.extend_from_slice(&available[..count]);
                reader.consume(count);
                if newline.is_some() {
                    complete = true;
                    break;
                }
            }

            if stop_without_response || (ended && line.is_empty()) {
                break;
            }

            if !complete {
                send_response(
                    output.as_ref(),
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
                    send_response(
                        output.as_ref(),
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
                    send_response(
                        output.as_ref(),
                        &mut stream,
                        &ServerMessage::Rejected {
                            reason: "Mensaje JSON inválido".to_string(),
                        },
                    )?;
                    break;
                }
            };

            if player_id.is_none() && !matches!(message, ClientMessage::Join { .. }) {
                send_response(
                    output.as_ref(),
                    &mut stream,
                    &ServerMessage::Rejected {
                        reason: "Debes ingresar antes de enviar acciones".to_string(),
                    },
                )?;
                break;
            }

            match message {
                ClientMessage::Join { version, name } => {
                    if player_id.is_some() {
                        send_response(
                            output.as_ref(),
                            &mut stream,
                            &ServerMessage::Rejected {
                                reason: "Ya estás conectado".to_string(),
                            },
                        )?;
                        continue;
                    }

                    if version != PROTOCOL_VERSION {
                        send_response(
                            output.as_ref(),
                            &mut stream,
                            &ServerMessage::Rejected {
                                reason: "Versión de protocolo incompatible".to_string(),
                            },
                        )?;
                        break;
                    }

                    let lifecycle_guard = if let Some(connections) = active_connections.as_ref() {
                        let guard = connections
                            .lifecycle
                            .lock()
                            .map_err(|_| io::Error::other("Ciclo de vida no disponible"))?;
                        if connections.is_closing() || accepted_at.elapsed() >= join_timeout {
                            drop(guard);
                            break;
                        }
                        Some(guard)
                    } else {
                        None
                    };

                    let publication = if output.is_some() {
                        Some(
                            state
                                .lock()
                                .map_err(|_| io::Error::other("Estado del servidor no disponible"))?
                                .publication
                                .clone(),
                        )
                    } else {
                        None
                    };
                    let publication_guard = if let Some(publication) = &publication {
                        Some(
                            publication
                                .lock()
                                .map_err(|_| io::Error::other("Publicación no disponible"))?,
                        )
                    } else {
                        None
                    };

                    let mut server = state
                        .lock()
                        .map_err(|_| io::Error::other("Estado del servidor no disponible"))?;

                    let id = match server.join(name.clone()) {
                        Ok(id) => id,
                        Err(reason) => {
                            drop(server);

                            send_response(
                                output.as_ref(),
                                &mut stream,
                                &ServerMessage::Rejected { reason },
                            )?;

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

                    let connection = Connection::new(stream.try_clone()?, output.clone());
                    server.connections.insert(id, connection);

                    if output.is_some() {
                        let enqueue_result = (|| -> io::Result<()> {
                            send_response(
                                output.as_ref(),
                                &mut stream,
                                &ServerMessage::Welcome {
                                    player_id: id,
                                    max_players: MAX_PLAYERS,
                                },
                            )?;
                            for (other_id, other_name) in &existing {
                                send_response(
                                    output.as_ref(),
                                    &mut stream,
                                    &ServerMessage::PlayerJoined {
                                        player_id: *other_id,
                                        name: other_name.clone(),
                                    },
                                )?;
                            }
                            Ok(())
                        })();
                        if let Err(error) = enqueue_result {
                            server.leave(id);
                            drop(server);
                            drop(publication_guard);
                            return Err(error);
                        }
                    }
                    drop(server);
                    if let Some(pending) = pending_registration.as_mut() {
                        pending.release();
                    }
                    if output.is_none() {
                        send_response(
                            output.as_ref(),
                            &mut stream,
                            &ServerMessage::Welcome {
                                player_id: id,
                                max_players: MAX_PLAYERS,
                            },
                        )?;
                        for (other_id, other_name) in existing {
                            send_response(
                                output.as_ref(),
                                &mut stream,
                                &ServerMessage::PlayerJoined {
                                    player_id: other_id,
                                    name: other_name,
                                },
                            )?;
                        }
                    }
                    let joined_message = ServerMessage::PlayerJoined {
                        player_id: id,
                        name: name.trim().to_string(),
                    };
                    if publication_guard.is_some() {
                        broadcast_message_locked(
                            &state,
                            events.as_ref(),
                            &joined_message,
                            Some(id),
                        );
                        if let Some(events) = &events {
                            events.push(ServerEvent::PlayerJoined {
                                player_id: id,
                                name: name.trim().to_string(),
                            });
                        }
                    } else {
                        broadcast_all(&state, events.as_ref(), &joined_message, Some(id));
                        if let Some(events) = &events {
                            events.push(ServerEvent::PlayerJoined {
                                player_id: id,
                                name: name.trim().to_string(),
                            });
                        }
                    }
                    drop(publication_guard);
                    drop(lifecycle_guard);

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
                                broadcast_all(&state, events.as_ref(), &message, Some(id));
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
                            .and_then(|connection| connection.output.clone());

                        if let Some(output) = connection {
                            if let Err(error) = output.enqueue(&ServerMessage::Pong) {
                                disconnect_player(&state, events.as_ref(), id);
                                return Err(error);
                            }
                        } else {
                            send_response(output.as_ref(), &mut stream, &ServerMessage::Pong)?;
                        }
                    } else {
                        send_response(output.as_ref(), &mut stream, &ServerMessage::Pong)?;
                    }
                }
            }

            line.clear();
        }

        Ok(())
    })();

    if let Some(id) = player_id {
        if disconnect_player(&state, events.as_ref(), id) {
            let count = state.lock().map(|server| server.count()).unwrap_or(0);
            println!("Jugador {id} desconectado. Jugadores activos: {count}");
        }
    }

    if let Some(output) = output {
        output.finish();
    }
    if let Some(writer) = writer {
        let _ = writer.join();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_server;
    use crate::sync::{NetworkOrientation, NetworkVec3};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::thread;

    fn start_test_server(
        join_timeout: Duration,
    ) -> (String, Arc<AtomicBool>, JoinHandle<io::Result<()>>) {
        let server = GameServer::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap().to_string();
        let shutdown = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            server.run_until_with_events_for_test(&signal, None, join_timeout)
        });
        (address, shutdown, handle)
    }

    fn start_test_server_with_events(
        join_timeout: Duration,
    ) -> (
        String,
        Arc<AtomicBool>,
        JoinHandle<io::Result<()>>,
        ServerEventQueue,
    ) {
        let server = GameServer::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap().to_string();
        let events = server.event_queue();
        let shutdown = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&shutdown);
        let server_events = events.clone();
        let handle = thread::spawn(move || {
            server.run_until_with_events_for_test(&signal, Some(server_events), join_timeout)
        });
        (address, shutdown, handle, events)
    }

    fn stop_test_server(shutdown: Arc<AtomicBool>, handle: JoinHandle<io::Result<()>>) {
        shutdown.store(true, Ordering::Release);
        assert!(handle.join().unwrap().is_ok());
    }

    #[test]
    fn output_queue_preserves_fifo_and_rejects_the_33rd_message() {
        let output = OutputQueue::new();
        for player_id in 0..3_u64 {
            output
                .enqueue(&ServerMessage::PlayerLeft { player_id })
                .unwrap();
        }

        for player_id in 0..3_u64 {
            let bytes = output.next().unwrap();
            let message = decode_server(std::str::from_utf8(&bytes).unwrap().trim()).unwrap();
            assert_eq!(message, ServerMessage::PlayerLeft { player_id });
        }

        for player_id in 0..MAX_PENDING_OUTPUT_MESSAGES as u64 {
            output
                .enqueue(&ServerMessage::PlayerLeft { player_id })
                .unwrap();
        }
        assert!(output
            .enqueue(&ServerMessage::PlayerLeft { player_id: 32 })
            .is_err());
        assert!(output.next().is_none());
    }

    #[test]
    fn concurrent_output_producers_publish_complete_messages() {
        let output = OutputQueue::new();
        let mut producers = Vec::new();
        for producer in 0..4_u64 {
            let output = Arc::clone(&output);
            producers.push(thread::spawn(move || {
                for offset in 0..8_u64 {
                    output
                        .enqueue(&ServerMessage::PlayerLeft {
                            player_id: producer * 8 + offset,
                        })
                        .unwrap();
                }
            }));
        }
        for producer in producers {
            producer.join().unwrap();
        }

        let mut player_ids = Vec::new();
        for _ in 0..32 {
            let bytes = output.next().unwrap();
            let message = decode_server(std::str::from_utf8(&bytes).unwrap().trim()).unwrap();
            if let ServerMessage::PlayerLeft { player_id } = message {
                player_ids.push(player_id);
            }
        }
        player_ids.sort_unstable();
        assert_eq!(player_ids, (0..32).collect::<Vec<_>>());
    }

    #[test]
    fn output_writer_sends_fifo_messages_without_interleaving_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let output = OutputQueue::new();
        let writer_output = Arc::clone(&output);
        let writer = thread::spawn(move || run_writer(server, writer_output));

        output
            .enqueue(&ServerMessage::Welcome {
                player_id: 7,
                max_players: MAX_PLAYERS,
            })
            .unwrap();
        output
            .enqueue(&ServerMessage::PlayerLeft { player_id: 7 })
            .unwrap();
        output.finish();

        let mut reader = BufReader::new(client);
        let mut first = String::new();
        let mut second = String::new();
        reader.read_line(&mut first).unwrap();
        reader.read_line(&mut second).unwrap();
        assert!(matches!(
            decode_server(first.trim()).unwrap(),
            ServerMessage::Welcome { .. }
        ));
        assert_eq!(
            decode_server(second.trim()).unwrap(),
            ServerMessage::PlayerLeft { player_id: 7 }
        );
        writer.join().unwrap();
    }

    #[test]
    fn publication_lock_keeps_welcome_before_later_broadcasts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server_stream, _) = listener.accept().unwrap();
        let output = OutputQueue::new();
        let state = Arc::new(Mutex::new(ServerState::default()));
        let player_id = 1;
        {
            let mut server = state.lock().unwrap();
            server.players.insert(player_id, "Jugador".to_string());
            server.connections.insert(
                player_id,
                Connection::new(server_stream, Some(Arc::clone(&output))),
            );
        }

        let publication = Arc::clone(&state.lock().unwrap().publication);
        let guard = publication.lock().unwrap();
        let state_for_broadcast = Arc::clone(&state);
        let broadcaster = thread::spawn(move || {
            broadcast_all(
                &state_for_broadcast,
                None,
                &ServerMessage::PlayerMoved {
                    player_id,
                    sequence: 1,
                    transform: PlayerTransform::new(
                        NetworkVec3::new(1.0, 0.0, 0.0),
                        NetworkOrientation::new(0.0, 0.0),
                        NetworkVec3::new(1.0, 0.0, 0.0),
                    ),
                },
                None,
            );
        });
        output
            .enqueue(&ServerMessage::Welcome {
                player_id,
                max_players: MAX_PLAYERS,
            })
            .unwrap();
        drop(guard);
        broadcaster.join().unwrap();

        let first = output.next().unwrap();
        let second = output.next().unwrap();
        assert!(matches!(
            decode_server(std::str::from_utf8(&first).unwrap().trim()).unwrap(),
            ServerMessage::Welcome { .. }
        ));
        assert!(matches!(
            decode_server(std::str::from_utf8(&second).unwrap().trim()).unwrap(),
            ServerMessage::PlayerMoved { .. }
        ));
        drop(client);
    }

    #[test]
    fn stale_movement_is_not_enqueued_after_player_left() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let healthy_client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (healthy_server, _) = listener.accept().unwrap();
        let retired_client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (retired_server, _) = listener.accept().unwrap();
        let healthy_output = OutputQueue::new();
        let retired_output = OutputQueue::new();
        let state = Arc::new(Mutex::new(ServerState::default()));
        let retired_id = 1;
        let healthy_id = 2;
        {
            let mut server = state.lock().unwrap();
            server.players.insert(retired_id, "Retirado".to_string());
            server.players.insert(healthy_id, "Sano".to_string());
            server.connections.insert(
                retired_id,
                Connection::new(retired_server, Some(retired_output)),
            );
            server.connections.insert(
                healthy_id,
                Connection::new(healthy_server, Some(Arc::clone(&healthy_output))),
            );
        }

        let events = ServerEventQueue::new();
        assert!(disconnect_player(&state, Some(&events), retired_id));
        broadcast_all(
            &state,
            Some(&events),
            &ServerMessage::PlayerMoved {
                player_id: retired_id,
                sequence: 9,
                transform: PlayerTransform::new(
                    NetworkVec3::new(9.0, 0.0, 0.0),
                    NetworkOrientation::new(0.0, 0.0),
                    NetworkVec3::new(1.0, 0.0, 0.0),
                ),
            },
            None,
        );

        let message = healthy_output.next().unwrap();
        assert_eq!(
            decode_server(std::str::from_utf8(&message).unwrap().trim()).unwrap(),
            ServerMessage::PlayerLeft {
                player_id: retired_id
            }
        );
        healthy_output.finish();
        assert!(healthy_output.next().is_none());
        drop(healthy_client);
        drop(retired_client);
    }

    fn send_join(stream: &mut TcpStream, name: &str) {
        let message = crate::encode(&ClientMessage::Join {
            version: PROTOCOL_VERSION,
            name: name.to_string(),
        })
        .unwrap();
        stream.write_all(message.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();
    }

    fn receive_message(stream: &TcpStream) -> ServerMessage {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        crate::decode_server(line.trim()).unwrap()
    }

    #[test]
    fn test_pending_timeout_releases_capacity_and_preserves_partial_join() {
        let (address, shutdown, handle) = start_test_server(Duration::from_millis(400));
        let mut idle = TcpStream::connect(&address).unwrap();
        idle.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut buffer = [0_u8; 1];
        assert_eq!(idle.read(&mut buffer).unwrap(), 0);

        let mut fragmented = TcpStream::connect(&address).unwrap();
        fragmented
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let message = crate::encode(&ClientMessage::Join {
            version: PROTOCOL_VERSION,
            name: "Fragmentado".to_string(),
        })
        .unwrap();
        let split = message.len() / 2;
        fragmented.write_all(message[..split].as_bytes()).unwrap();
        thread::sleep(Duration::from_millis(150));
        fragmented.write_all(message[split..].as_bytes()).unwrap();
        fragmented.write_all(b"\n").unwrap();
        assert!(matches!(
            receive_message(&fragmented),
            ServerMessage::Welcome { .. }
        ));

        fragmented.shutdown(Shutdown::Both).unwrap();
        idle.shutdown(Shutdown::Both).unwrap();
        stop_test_server(shutdown, handle);
    }

    #[test]
    fn test_pending_capacity_rejects_fourth_and_accepts_after_release() {
        let (address, shutdown, handle) = start_test_server(Duration::from_secs(1));
        let pending = (0..MAX_PENDING_CONNECTIONS)
            .map(|_| TcpStream::connect(&address).unwrap())
            .collect::<Vec<_>>();

        let fourth = TcpStream::connect(&address).unwrap();
        fourth
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut buffer = [0_u8; 1];
        assert_eq!(fourth.try_clone().unwrap().read(&mut buffer).unwrap(), 0);

        pending[0].shutdown(Shutdown::Both).unwrap();
        let mut replacement = None;
        for _ in 0..20 {
            let mut candidate = TcpStream::connect(&address).unwrap();
            candidate
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            send_join(&mut candidate, "Reemplazo");
            let message = receive_message(&candidate);
            if matches!(message, ServerMessage::Welcome { .. }) {
                replacement = Some(candidate);
                break;
            }
            let _ = candidate.shutdown(Shutdown::Both);
            thread::sleep(Duration::from_millis(10));
        }
        assert!(replacement.is_some());

        for stream in pending {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Some(stream) = replacement {
            let _ = stream.shutdown(Shutdown::Both);
        }
        stop_test_server(shutdown, handle);
    }

    #[test]
    fn test_join_after_pending_deadline_does_not_create_player() {
        let (address, shutdown, handle, events) =
            start_test_server_with_events(Duration::from_millis(100));
        let mut client = TcpStream::connect(&address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let message = crate::encode(&ClientMessage::Join {
            version: PROTOCOL_VERSION,
            name: "Tardío".to_string(),
        })
        .unwrap();
        let split = message.len() / 2;
        client.write_all(message[..split].as_bytes()).unwrap();
        thread::sleep(Duration::from_millis(150));
        let _ = client.write_all(message[split..].as_bytes());
        let _ = client.write_all(b"\n");
        let mut buffer = [0_u8; 1];
        match client.read(&mut buffer) {
            Ok(0) => {}
            Err(error) => assert!(matches!(
                error.kind(),
                io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::UnexpectedEof
            )),
            Ok(bytes) => panic!("se recibieron {bytes} bytes después del plazo"),
        }
        assert!(events.pop().is_none());
        stop_test_server(shutdown, handle);
    }

    #[test]
    fn test_reap_finished_client_threads_keeps_active_handlers() {
        let finished = thread::spawn(|| {});
        while !finished.is_finished() {
            thread::yield_now();
        }
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let (sender, receiver) = std::sync::mpsc::channel();
        let active = thread::spawn(move || {
            started_sender.send(()).unwrap();
            receiver.recv().unwrap();
        });
        started_receiver.recv().unwrap();
        let mut threads = vec![active, finished];
        let mut error = None;

        reap_finished_client_threads(&mut threads, &mut error);
        assert_eq!(threads.len(), 1);
        assert!(error.is_none());

        sender.send(()).unwrap();
        for _ in 0..100 {
            if threads[0].is_finished() {
                break;
            }
            thread::yield_now();
        }
        reap_finished_client_threads(&mut threads, &mut error);
        assert!(threads.is_empty());
        assert!(error.is_none());
    }

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
    fn test_simultaneous_broadcast_failures_clean_each_player_once() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let healthy_client = TcpStream::connect(address).unwrap();
        let (healthy_server, _) = listener.accept().unwrap();
        healthy_client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let failed_client = TcpStream::connect(address).unwrap();
        let (failed_server, _) = listener.accept().unwrap();
        failed_server.shutdown(Shutdown::Both).unwrap();
        let failed_client_2 = TcpStream::connect(address).unwrap();
        let (failed_server_2, _) = listener.accept().unwrap();
        failed_server_2.shutdown(Shutdown::Both).unwrap();

        let healthy_output = OutputQueue::new();
        let healthy_writer_output = Arc::clone(&healthy_output);
        let healthy_writer_stream = healthy_server.try_clone().unwrap();
        let healthy_writer =
            thread::spawn(move || run_writer(healthy_writer_stream, healthy_writer_output));
        let failed_output = OutputQueue::new();
        failed_output.close();
        let failed_output_2 = OutputQueue::new();
        failed_output_2.close();

        let mut initial = ServerState::default();
        let healthy_id = initial.join("Jugador sano".to_string()).unwrap();
        let failed_id = initial.join("Jugador fallido".to_string()).unwrap();
        let failed_id_2 = initial.join("Jugador fallido 2".to_string()).unwrap();
        initial.connections.insert(
            healthy_id,
            Connection::new(healthy_server, Some(healthy_output)),
        );
        initial.connections.insert(
            failed_id,
            Connection::new(failed_server, Some(failed_output)),
        );
        initial.connections.insert(
            failed_id_2,
            Connection::new(failed_server_2, Some(failed_output_2)),
        );
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
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            crate::decode_server(line.trim()).unwrap(),
            ServerMessage::PlayerLeft {
                player_id: failed_id,
            }
        );
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            crate::decode_server(line.trim()).unwrap(),
            ServerMessage::PlayerLeft {
                player_id: failed_id_2,
            }
        );

        let server = state.lock().unwrap();
        assert!(server.players.contains_key(&healthy_id));
        assert!(!server.players.contains_key(&failed_id));
        assert!(!server.players.contains_key(&failed_id_2));
        assert!(!server.connections.contains_key(&failed_id));
        assert!(!server.connections.contains_key(&failed_id_2));
        drop(server);

        let mut disconnected = vec![events.pop().unwrap().clone(), events.pop().unwrap().clone()];
        disconnected.sort_by_key(|event| match event {
            ServerEvent::PlayerLeft { player_id } => *player_id,
            _ => 0,
        });
        assert_eq!(
            disconnected,
            vec![
                ServerEvent::PlayerLeft {
                    player_id: failed_id,
                },
                ServerEvent::PlayerLeft {
                    player_id: failed_id_2,
                },
            ]
        );
        assert_eq!(events.pop(), None);
        assert!(!disconnect_player(&state, Some(&events), failed_id));
        assert!(!disconnect_player(&state, Some(&events), failed_id_2));
        if let Some(connection) = state.lock().unwrap().connections.get(&healthy_id) {
            if let Some(output) = &connection.output {
                output.close();
            }
        }
        healthy_writer.join().unwrap();
        drop(failed_client);
        drop(failed_client_2);
    }

    #[test]
    fn test_full_queue_during_disconnect_broadcast_is_idempotent_and_joinable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let healthy_client = TcpStream::connect(address).unwrap();
        let (healthy_server, _) = listener.accept().unwrap();
        let failed_client = TcpStream::connect(address).unwrap();
        let (failed_server, _) = listener.accept().unwrap();
        failed_server.shutdown(Shutdown::Both).unwrap();

        let healthy_output = OutputQueue::new();
        let healthy_writer = {
            let stream = healthy_server.try_clone().unwrap();
            let output = Arc::clone(&healthy_output);
            thread::spawn(move || run_writer(stream, output))
        };
        let failed_output = OutputQueue::new();
        for player_id in 0..MAX_PENDING_OUTPUT_MESSAGES as u64 {
            failed_output
                .enqueue(&ServerMessage::PlayerLeft { player_id })
                .unwrap();
        }
        assert!(failed_output
            .enqueue(&ServerMessage::PlayerLeft {
                player_id: MAX_PENDING_OUTPUT_MESSAGES as u64,
            })
            .is_err());

        let mut initial = ServerState::default();
        let healthy_id = initial.join("Jugador sano".to_string()).unwrap();
        let failed_id = initial.join("Jugador con cola llena".to_string()).unwrap();
        initial.connections.insert(
            healthy_id,
            Connection::new(healthy_server, Some(Arc::clone(&healthy_output))),
        );
        initial.connections.insert(
            failed_id,
            Connection::new(failed_server, Some(failed_output)),
        );
        let state = Arc::new(Mutex::new(initial));
        let events = ServerEventQueue::new();

        broadcast_all(
            &state,
            Some(&events),
            &ServerMessage::PlayerJoined {
                player_id: healthy_id,
                name: "Jugador sano".to_string(),
            },
            None,
        );

        let mut reader = BufReader::new(healthy_client);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            decode_server(line.trim()).unwrap(),
            ServerMessage::PlayerJoined {
                player_id: healthy_id,
                name: "Jugador sano".to_string(),
            }
        );
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            decode_server(line.trim()).unwrap(),
            ServerMessage::PlayerLeft {
                player_id: failed_id,
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

        healthy_output.close();
        healthy_writer.join().unwrap();
        drop(failed_client);
    }

    #[test]
    fn test_controlled_write_cancellation_is_joinable() {
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let (cancel_sender, cancel_receiver) = std::sync::mpsc::channel();
        let writer = thread::spawn(move || {
            ControlledWrite {
                started: started_sender,
                cancel: cancel_receiver,
            }
            .run()
        });

        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("la escritura controlada no alcanzó el punto de bloqueo");
        cancel_sender
            .send(())
            .expect("el cierre de prueba no pudo cancelar la escritura");

        let result = writer
            .join()
            .expect("el hilo de escritura controlada no terminó");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }
}
