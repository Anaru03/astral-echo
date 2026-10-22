use serde::{Deserialize, Serialize};

pub const MAX_PLAYERS: usize = 3;
pub const PROTOCOL_VERSION: u32 = 1;

pub type NetworkPlayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Join {
        version: u32,
        name: String,
    },
    Leave,
    Ping,
    PlayerMove {
        sequence: u64,
        transform: crate::sync::PlayerTransform,
    },
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
    PlayerMoved {
        player_id: NetworkPlayerId,
        sequence: u64,
        transform: crate::sync::PlayerTransform,
    },
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
    use crate::sync::{NetworkOrientation, NetworkVec3, PlayerTransform};

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
    fn test_unknown_client_fields_are_rejected() {
        let result = decode_client(
            r#"{"type":"Join","data":{"version":1,"name":"Jugador","unexpected":"not allowed"}}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_ping_pong() {
        let ping = ClientMessage::Ping;
        let pong = ServerMessage::Pong;

        assert_eq!(decode_client(&encode(&ping).unwrap()).unwrap(), ping);

        assert_eq!(decode_server(&encode(&pong).unwrap()).unwrap(), pong);
    }

    #[test]
    fn test_player_move_message() {
        let transform = PlayerTransform::new(
            NetworkVec3::new(1.0, 2.0, 3.0),
            NetworkOrientation::new(90.0, 15.0),
            NetworkVec3::new(0.0, 0.0, 1.0),
        );

        let message = ClientMessage::PlayerMove {
            sequence: 1,
            transform,
        };

        let json = encode(&message).unwrap();
        let decoded = decode_client(&json).unwrap();

        assert_eq!(message, decoded);
    }

    #[test]
    fn test_player_moved_message() {
        let transform = PlayerTransform::new(
            NetworkVec3::new(5.0, 0.0, 2.0),
            NetworkOrientation::new(45.0, 0.0),
            NetworkVec3::new(1.0, 0.0, 0.0),
        );

        let message = ServerMessage::PlayerMoved {
            player_id: 2,
            sequence: 10,
            transform,
        };

        let json = encode(&message).unwrap();
        let decoded = decode_server(&json).unwrap();

        assert_eq!(message, decoded);
    }
}