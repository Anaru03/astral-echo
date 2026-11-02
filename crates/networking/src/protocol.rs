use serde::{Deserialize, Serialize};

pub const MAX_PLAYERS: usize = 3;
pub const PROTOCOL_VERSION: u32 = 2;

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
    RequestMissionRecovery {
        request_id: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MissionRecoveryOutcome {
    Succeeded,
    ContentUnavailable,
    InvalidMissionTransition,
    Unauthorized,
    InvalidRequest,
    Duplicate,
    SessionUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MissionStateProjection {
    Locked,
    Available,
    Active,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ObjectiveStateProjection {
    Locked,
    Available,
    Active,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mission5ObjectiveProjection {
    pub objective_id: u32,
    pub required: bool,
    pub state: ObjectiveStateProjection,
    pub progress: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Mission5DestinationTypeProjection {
    Planet,
    SpacePoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Mission5DiscoveryProjection {
    Unknown,
    Known,
    Discovered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mission5DestinationProjection {
    pub destination_id: u32,
    pub destination_type: Mission5DestinationTypeProjection,
    pub position: [f32; 3],
    pub travel_duration: u64,
    pub fuel_cost: u32,
    pub discovery: Mission5DiscoveryProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mission5ShooterProjection {
    pub entity_id: u64,
    pub active: bool,
    pub current_health: u32,
    pub maximum_health: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mission5ContentProjection {
    pub mission_id: u32,
    pub state: MissionStateProjection,
    pub required_completed: u32,
    pub required_total: u32,
    pub optional_completed: u32,
    pub optional_total: u32,
    pub objectives: Vec<Mission5ObjectiveProjection>,
    pub destination: Option<Mission5DestinationProjection>,
    pub shooter: Option<Mission5ShooterProjection>,
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
    MissionRecoveryResult {
        request_id: u64,
        outcome: MissionRecoveryOutcome,
    },
    MissionContentUpdated {
        content: Mission5ContentProjection,
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

    #[test]
    fn test_mission_recovery_request_and_response() {
        let request = ClientMessage::RequestMissionRecovery { request_id: 42 };
        let response = ServerMessage::MissionRecoveryResult {
            request_id: 42,
            outcome: MissionRecoveryOutcome::ContentUnavailable,
        };

        assert_eq!(decode_client(&encode(&request).unwrap()).unwrap(), request);
        assert_eq!(decode_server(&encode(&response).unwrap()).unwrap(), response);
    }

    #[test]
    fn test_mission_recovery_request_rejects_unexpected_fields() {
        let result = decode_client(
            r#"{"type":"RequestMissionRecovery","data":{"request_id":1,"player_id":9}}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_mission_content_projection_round_trips() {
        let message = ServerMessage::MissionContentUpdated {
            content: Mission5ContentProjection {
                mission_id: 5,
                state: MissionStateProjection::Active,
                required_completed: 0,
                required_total: 3,
                optional_completed: 0,
                optional_total: 0,
                objectives: vec![Mission5ObjectiveProjection {
                    objective_id: 1,
                    required: true,
                    state: ObjectiveStateProjection::Active,
                    progress: 0,
                }],
                destination: Some(Mission5DestinationProjection {
                    destination_id: 104,
                    destination_type: Mission5DestinationTypeProjection::SpacePoint,
                    position: [0.0, 0.0, 0.0],
                    travel_duration: 100,
                    fuel_cost: 1,
                    discovery: Mission5DiscoveryProjection::Unknown,
                }),
                shooter: Some(Mission5ShooterProjection {
                    entity_id: 9,
                    active: true,
                    current_health: 20,
                    maximum_health: 20,
                }),
            },
        };

        assert_eq!(decode_server(&encode(&message).unwrap()).unwrap(), message);
    }
}