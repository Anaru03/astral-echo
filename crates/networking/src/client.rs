use crate::protocol::{
    decode_server, encode, ClientMessage, NetworkPlayerId, ServerMessage, PROTOCOL_VERSION,
};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const MAX_MESSAGE_SIZE: u64 = 4096;

pub struct GameClient {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    player_id: NetworkPlayerId,
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

    pub fn receive(&mut self) -> io::Result<ServerMessage> {
        let mut line = String::new();

        let bytes = self
            .reader
            .by_ref()
            .take(MAX_MESSAGE_SIZE + 1)
            .read_line(&mut line)?;

        if bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "El servidor cerró la conexión",
            ));
        }

        if bytes as u64 > MAX_MESSAGE_SIZE || !line.ends_with('\n') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Mensaje demasiado largo o incompleto",
            ));
        }

        decode_server(line.trim_end())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn ping(&mut self) -> io::Result<()> {
        self.send(&ClientMessage::Ping)?;

        match self.receive()? {
            ServerMessage::Pong => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Se esperaba una respuesta Pong",
            )),
        }
    }

    pub fn disconnect(&mut self) -> io::Result<()> {
        self.send(&ClientMessage::Leave)
    }
}
