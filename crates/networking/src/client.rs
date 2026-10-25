use crate::protocol::{
    decode_server, encode, ClientMessage, NetworkPlayerId, ServerMessage, PROTOCOL_VERSION,
};

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

const MAX_MESSAGE_SIZE: usize = 4096;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct GameClient {
    stream: TcpStream,
    player_id: NetworkPlayerId,
    pending_events: VecDeque<ServerMessage>,
    read_buffer: Vec<u8>,
    disconnected: bool,
}

impl GameClient {
    pub fn connect<A: ToSocketAddrs>(address: A, name: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;

        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        stream.set_nodelay(true)?;

        let mut client = Self {
            stream,
            player_id: 0,
            pending_events: VecDeque::new(),
            read_buffer: Vec::new(),
            disconnected: false,
        };

        client.send(&ClientMessage::Join {
            version: PROTOCOL_VERSION,
            name: name.to_string(),
        })?;

        match client.receive()? {
            ServerMessage::Welcome { player_id, .. } => {
                client.player_id = player_id;
                Ok(client)
            }

            ServerMessage::Rejected { reason } => {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, reason))
            }

            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Respuesta inesperada del servidor",
            )),
        }
    }

    pub fn player_id(&self) -> NetworkPlayerId {
        self.player_id
    }

    pub fn send(&mut self, message: &ClientMessage) -> io::Result<()> {
        if self.disconnected {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "El cliente está desconectado",
            ));
        }

        let json =
            encode(message).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        if json.len() + 1 > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Mensaje demasiado largo",
            ));
        }

        self.stream.write_all(json.as_bytes())?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()
    }

    fn extract_message(&mut self) -> io::Result<Option<ServerMessage>> {
        let Some(newline) = self.read_buffer.iter().position(|byte| *byte == b'\n') else {
            if self.read_buffer.len() >= MAX_MESSAGE_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Mensaje demasiado largo",
                ));
            }

            return Ok(None);
        };

        let message_length = newline + 1;

        if message_length > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Mensaje demasiado largo",
            ));
        }

        let bytes = self.read_buffer.drain(..message_length).collect::<Vec<_>>();

        let text = std::str::from_utf8(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        let message = decode_server(text.trim_end())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        Ok(Some(message))
    }

    fn receive_from_network(
        &mut self,
        timeout: Option<Duration>,
    ) -> io::Result<Option<ServerMessage>> {
        if let Some(message) = self.extract_message()? {
            return Ok(Some(message));
        }

        if self.disconnected {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "El cliente está desconectado",
            ));
        }

        let deadline = timeout.map(|duration| Instant::now() + duration);
        let mut attempted_read = false;

        loop {
            if let Some(message) = self.extract_message()? {
                return Ok(Some(message));
            }

            // Incluso con Duration::ZERO debemos intentar leer una vez.
            // De otro modo try_receive() devolveria siempre None.
            if attempted_read {
                if let Some(deadline) = deadline {
                    if Instant::now() >= deadline {
                        return Ok(None);
                    }
                }
            }
            attempted_read = true;

            let mut buffer = [0_u8; 1024];

            // En modo no bloqueante, la lectura devuelve WouldBlock
            // cuando todavía no hay información disponible.
            self.stream.set_nonblocking(true)?;

            let result = self.stream.read(&mut buffer);

            // Restaurar el modo bloqueante para las escrituras.
            self.stream.set_nonblocking(false)?;

            match result {
                Ok(0) => {
                    self.disconnected = true;

                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "El servidor cerró la conexión",
                    ));
                }

                Ok(bytes) => {
                    self.read_buffer.extend_from_slice(&buffer[..bytes]);

                    // Detectar mensajes demasiado grandes incluso
                    // cuando todavía no llega el salto de línea.
                    let first_line_length = self
                        .read_buffer
                        .iter()
                        .position(|byte| *byte == b'\n')
                        .map(|position| position + 1)
                        .unwrap_or(self.read_buffer.len());

                    if first_line_length > MAX_MESSAGE_SIZE {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Mensaje demasiado largo",
                        ));
                    }
                }

                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.kind() == io::ErrorKind::TimedOut =>
                {
                    if timeout.is_none() {
                        std::thread::sleep(Duration::from_millis(1));
                    } else {
                        return Ok(None);
                    }
                }

                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    continue;
                }

                Err(error) => return Err(error),
            }
        }
    }

    pub fn receive(&mut self) -> io::Result<ServerMessage> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(event);
        }

        let deadline = Instant::now() + READ_TIMEOUT;

        loop {
            let now = Instant::now();

            if now >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Tiempo de espera agotado",
                ));
            }

            match self.receive_from_network(Some(deadline - now))? {
                Some(message) => return Ok(message),
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }

    pub fn try_receive(&mut self) -> io::Result<Option<ServerMessage>> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        self.receive_from_network(Some(Duration::ZERO))
    }

    pub fn ping(&mut self) -> io::Result<()> {
        self.send(&ClientMessage::Ping)?;

        let deadline = Instant::now() + READ_TIMEOUT;

        loop {
            let now = Instant::now();

            if now >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "No se recibió Pong",
                ));
            }

            match self.receive_from_network(Some(deadline - now))? {
                Some(ServerMessage::Pong) => return Ok(()),

                Some(
                    event @ (ServerMessage::PlayerJoined { .. }
                    | ServerMessage::PlayerLeft { .. }
                    | ServerMessage::PlayerMoved { .. }),
                ) => {
                    self.pending_events.push_back(event);
                }

                Some(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Respuesta inesperada al Ping",
                    ));
                }

                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }

    pub fn disconnect(&mut self) -> io::Result<()> {
        if self.disconnected {
            return Ok(());
        }

        self.send(&ClientMessage::Leave)?;
        self.disconnected = true;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn test_client(stream: TcpStream) -> GameClient {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        GameClient {
            stream,
            player_id: 1,
            pending_events: VecDeque::new(),
            read_buffer: Vec::new(),
            disconnected: false,
        }
    }

    #[test]
    fn receive_rejects_oversized_server_message() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let writer = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            stream.write_all(&vec![b'A'; MAX_MESSAGE_SIZE + 1]).unwrap();

            stream.write_all(b"\n").unwrap();
        });

        let stream = TcpStream::connect(address).unwrap();
        let mut client = test_client(stream);

        let error = client.receive().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        writer.join().unwrap();
    }

    #[test]
    fn try_receive_returns_none_when_no_data() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(100));
            drop(stream);
        });

        let stream = TcpStream::connect(address).unwrap();
        let mut client = test_client(stream);

        assert!(client.try_receive().unwrap().is_none());

        server.join().unwrap();
    }

    #[test]
    fn try_receive_preserves_partial_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            let message = encode(&ServerMessage::PlayerJoined {
                player_id: 2,
                name: "Jugador 2".to_string(),
            })
            .unwrap();

            let split = message.len() / 2;

            stream.write_all(message[..split].as_bytes()).unwrap();

            thread::sleep(Duration::from_millis(100));

            stream.write_all(message[split..].as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
        });

        let stream = TcpStream::connect(address).unwrap();
        let mut client = test_client(stream);

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut received = None;

        while Instant::now() < deadline {
            if let Some(message) = client.try_receive().unwrap() {
                received = Some(message);
                break;
            }

            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            received,
            Some(ServerMessage::PlayerJoined {
                player_id: 2,
                name: "Jugador 2".to_string(),
            })
        );

        server.join().unwrap();
    }

    #[test]
    fn try_receive_preserves_pending_events() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(100));
            drop(stream);
        });

        let stream = TcpStream::connect(address).unwrap();
        let mut client = test_client(stream);

        client
            .pending_events
            .push_back(ServerMessage::PlayerJoined {
                player_id: 2,
                name: "Jugador 2".to_string(),
            });

        assert_eq!(
            client.try_receive().unwrap(),
            Some(ServerMessage::PlayerJoined {
                player_id: 2,
                name: "Jugador 2".to_string(),
            })
        );

        server.join().unwrap();
    }

    #[test]
    fn try_receive_detects_closed_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            drop(stream);
        });

        let stream = TcpStream::connect(address).unwrap();
        let mut client = test_client(stream);

        let deadline = Instant::now() + Duration::from_secs(2);

        loop {
            match client.try_receive() {
                Err(error) => {
                    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
                    break;
                }

                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }

                other => panic!("Resultado inesperado: {other:?}"),
            }
        }

        server.join().unwrap();
    }

    #[test]
    fn try_receive_handles_multiple_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            for id in [2, 3] {
                let message = encode(&ServerMessage::PlayerJoined {
                    player_id: id,
                    name: format!("Jugador {id}"),
                })
                .unwrap();

                stream.write_all(message.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });

        let stream = TcpStream::connect(address).unwrap();
        let mut client = test_client(stream);

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut received = Vec::new();

        while received.len() < 2 && Instant::now() < deadline {
            if let Some(message) = client.try_receive().unwrap() {
                received.push(message);
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }

        assert_eq!(received.len(), 2);

        assert_eq!(
            received[0],
            ServerMessage::PlayerJoined {
                player_id: 2,
                name: "Jugador 2".to_string(),
            }
        );

        assert_eq!(
            received[1],
            ServerMessage::PlayerJoined {
                player_id: 3,
                name: "Jugador 3".to_string(),
            }
        );

        server.join().unwrap();
    }
}