use crate::protocol::{
    decode_server, encode, ClientMessage, NetworkPlayerId, ServerMessage, PROTOCOL_VERSION,
};
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const MAX_MESSAGE_SIZE: usize = 4096;

pub struct GameClient {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    player_id: NetworkPlayerId,
    pending_events: VecDeque<ServerMessage>,
}

impl GameClient {
    pub fn connect<A: ToSocketAddrs>(address: A, name: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;

        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?;

        let reader = BufReader::new(stream.try_clone()?);

        let mut client = Self {
            stream,
            reader,
            player_id: 0,
            pending_events: VecDeque::new(),
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
        let json =
            encode(message).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        self.stream.write_all(json.as_bytes())?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()
    }

    fn receive_from_network(&mut self) -> io::Result<ServerMessage> {
        let mut line = Vec::new();

        let bytes = self
            .reader
            .by_ref()
            .take((MAX_MESSAGE_SIZE + 1) as u64)
            .read_until(b'\n', &mut line)?;

        if bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "El servidor cerró la conexión",
            ));
        }

        if bytes > MAX_MESSAGE_SIZE || !line.ends_with(b"\n") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Mensaje demasiado largo o incompleto",
            ));
        }

        let text = std::str::from_utf8(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        decode_server(text.trim_end())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn receive(&mut self) -> io::Result<ServerMessage> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(event);
        }

        self.receive_from_network()
    }

    pub fn ping(&mut self) -> io::Result<()> {
        self.send(&ClientMessage::Ping)?;

        loop {
            match self.receive_from_network()? {
                ServerMessage::Pong => return Ok(()),

                event @ (ServerMessage::PlayerJoined { .. }
                | ServerMessage::PlayerLeft { .. }
                | ServerMessage::PlayerMoved { .. }) => {
                    self.pending_events.push_back(event);
                }

                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Respuesta inesperada al Ping",
                    ));
                }
            }
        }
    }

    pub fn disconnect(&mut self) -> io::Result<()> {
        self.send(&ClientMessage::Leave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

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
        stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut client = GameClient {
            stream,
            reader,
            player_id: 0,
            pending_events: VecDeque::new(),
        };

        let error = client.receive().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        writer.join().unwrap();
    }
}