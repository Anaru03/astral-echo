use serde::{Deserialize, Serialize};

pub const MAX_PLAYERS: usize = 3;
pub const PROTOCOL_VERSION: u32 = 1;

pub type NetworkPlayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Join { version: u32, name: String },
    Leave,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome {
        player_id: NetworkPlayerId,
        max_players: usize,
    },
    PlayerJoined {
        player_id: NetworkPlayerId,
        name: String,
    },
    PlayerLeft {
        player_id: NetworkPlayerId,
    },
    Rejected {
        reason: String,
    },
    Pong,
}

#[derive(Debug)]
pub enum ProtocolError {
    Serialization(serde_json::Error),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<serde_json::Error> for ProtocolError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub fn encode<T: Serialize>(message: &T) -> Result<String, ProtocolError> {
    Ok(serde_json::to_string(message)?)
}

pub fn decode_client(data: &str) -> Result<ClientMessage, ProtocolError> {
    Ok(serde_json::from_str(data)?)
}

pub fn decode_server(data: &str) -> Result<ServerMessage, ProtocolError> {
    Ok(serde_json::from_str(data)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_message() {
        let message = ClientMessage::Join {
            version: PROTOCOL_VERSION,
            name: "Jugador 1".to_string(),
        };

        let json = encode(&message).unwrap();
        let decoded = decode_client(&json).unwrap();

        assert_eq!(message, decoded);
    }

    #[test]
    fn test_welcome_message() {
        let message = ServerMessage::Welcome {
            player_id: 1,
            max_players: MAX_PLAYERS,
        };

        let json = encode(&message).unwrap();
        let decoded = decode_server(&json).unwrap();

        assert_eq!(message, decoded);
    }

    #[test]
    fn test_player_joined_message() {
        let message = ServerMessage::PlayerJoined {
            player_id: 2,
            name: "Jugador 2".to_string(),
        };

        let json = encode(&message).unwrap();
        let decoded = decode_server(&json).unwrap();

        assert_eq!(message, decoded);
    }

    #[test]
    fn test_player_left_message() {
        let message = ServerMessage::PlayerLeft { player_id: 2 };

        let json = encode(&message).unwrap();
        let decoded = decode_server(&json).unwrap();

        assert_eq!(message, decoded);
    }

    #[test]
    fn test_invalid_message() {
        let result = decode_client("mensaje invalido");

        assert!(result.is_err());
    }

    #[test]
    fn test_ping_pong() {
        let ping = ClientMessage::Ping;
        let pong = ServerMessage::Pong;

        assert_eq!(decode_client(&encode(&ping).unwrap()).unwrap(), ping);
        assert_eq!(decode_server(&encode(&pong).unwrap()).unwrap(), pong);
    }
}
