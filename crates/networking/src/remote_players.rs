use crate::protocol::{NetworkPlayerId, ServerMessage};
use crate::sync::PlayerTransform;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePlayer {
    pub id: NetworkPlayerId,
    pub name: String,
    pub transform: Option<PlayerTransform>,
    pub last_sequence: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct RemotePlayers {
    players: BTreeMap<NetworkPlayerId, RemotePlayer>,
}

impl RemotePlayers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn get(&self, id: NetworkPlayerId) -> Option<&RemotePlayer> {
        self.players.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RemotePlayer> {
        self.players.values()
    }

    pub fn clear(&mut self) {
        self.players.clear();
    }

    pub fn apply(&mut self, message: ServerMessage) -> bool {
        match message {
            ServerMessage::PlayerJoined { player_id, name } => {
                if let Some(player) = self.players.get_mut(&player_id) {
                    // Una notificación repetida no debe borrar
                    // la última posición ni la secuencia recibida.
                    if player.name == name {
                        return false;
                    }

                    player.name = name;
                    true
                } else {
                    self.players.insert(
                        player_id,
                        RemotePlayer {
                            id: player_id,
                            name,
                            transform: None,
                            last_sequence: None,
                        },
                    );
                    true
                }
            }

            ServerMessage::PlayerMoved {
                player_id,
                sequence,
                transform,
            } => {
                if !transform.is_valid() {
                    return false;
                }

                let Some(player) = self.players.get_mut(&player_id) else {
                    // No crear jugadores a partir de movimientos
                    // de identificadores desconocidos.
                    return false;
                };

                if player
                    .last_sequence
                    .is_some_and(|previous| sequence <= previous)
                {
                    return false;
                }

                player.transform = Some(transform);
                player.last_sequence = Some(sequence);
                true
            }

            ServerMessage::PlayerLeft { player_id } => self.players.remove(&player_id).is_some(),

            // Welcome, Pong y Rejected no modifican
            // el estado de los jugadores remotos.
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{NetworkOrientation, NetworkVec3};

    fn sample_transform(x: f32) -> PlayerTransform {
        PlayerTransform::new(
            NetworkVec3::new(x, 2.0, 3.0),
            NetworkOrientation::new(90.0, 15.0),
            NetworkVec3::new(1.0, 0.0, 0.0),
        )
    }

    fn joined(id: NetworkPlayerId) -> ServerMessage {
        ServerMessage::PlayerJoined {
            player_id: id,
            name: format!("Jugador {id}"),
        }
    }

    fn moved(id: NetworkPlayerId, sequence: u64, x: f32) -> ServerMessage {
        ServerMessage::PlayerMoved {
            player_id: id,
            sequence,
            transform: sample_transform(x),
        }
    }

    #[test]
    fn test_initial_state_is_empty() {
        let players = RemotePlayers::new();

        assert!(players.is_empty());
        assert_eq!(players.len(), 0);
    }

    #[test]
    fn test_register_remote_player() {
        let mut players = RemotePlayers::new();

        assert!(players.apply(joined(1)));

        let player = players.get(1).unwrap();

        assert_eq!(player.id, 1);
        assert_eq!(player.name, "Jugador 1");
        assert_eq!(player.transform, None);
        assert_eq!(player.last_sequence, None);
    }

    #[test]
    fn test_register_multiple_players() {
        let mut players = RemotePlayers::new();

        assert!(players.apply(joined(1)));
        assert!(players.apply(joined(2)));
        assert!(players.apply(joined(3)));

        assert_eq!(players.len(), 3);

        let ids: Vec<_> = players.iter().map(|player| player.id).collect();

        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_update_player_transform() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));

        assert!(players.apply(moved(1, 1, 10.0)));

        let player = players.get(1).unwrap();

        assert_eq!(player.transform, Some(sample_transform(10.0)));
        assert_eq!(player.last_sequence, Some(1));
    }

    #[test]
    fn test_reject_outdated_movement() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));
        players.apply(moved(1, 10, 10.0));

        assert!(!players.apply(moved(1, 5, 20.0)));

        assert_eq!(
            players.get(1).unwrap().transform,
            Some(sample_transform(10.0))
        );
    }

    #[test]
    fn test_reject_duplicate_sequence() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));
        players.apply(moved(1, 10, 10.0));

        assert!(!players.apply(moved(1, 10, 50.0)));

        assert_eq!(players.get(1).unwrap().last_sequence, Some(10));
    }

    #[test]
    fn test_reject_unknown_player_movement() {
        let mut players = RemotePlayers::new();

        assert!(!players.apply(moved(99, 1, 10.0)));
        assert!(players.is_empty());
    }

    #[test]
    fn test_reject_invalid_transform() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));

        let invalid = PlayerTransform::new(
            NetworkVec3::new(f32::NAN, 0.0, 0.0),
            NetworkOrientation::new(0.0, 0.0),
            NetworkVec3::new(0.0, 0.0, 0.0),
        );

        assert!(!players.apply(ServerMessage::PlayerMoved {
            player_id: 1,
            sequence: 1,
            transform: invalid,
        }));

        assert_eq!(players.get(1).unwrap().transform, None);
    }

    #[test]
    fn test_remove_remote_player() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));
        players.apply(joined(2));

        assert!(players.apply(ServerMessage::PlayerLeft { player_id: 1 }));

        assert_eq!(players.len(), 1);
        assert!(players.get(1).is_none());
        assert!(players.get(2).is_some());
    }

    #[test]
    fn test_duplicate_join_preserves_transform() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));
        players.apply(moved(1, 5, 15.0));

        assert!(!players.apply(joined(1)));

        let player = players.get(1).unwrap();

        assert_eq!(player.transform, Some(sample_transform(15.0)));
        assert_eq!(player.last_sequence, Some(5));
    }

    #[test]
    fn test_reconnection_resets_player_state() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));
        players.apply(moved(1, 10, 10.0));

        players.apply(ServerMessage::PlayerLeft { player_id: 1 });

        players.apply(joined(1));

        let player = players.get(1).unwrap();

        assert_eq!(player.transform, None);
        assert_eq!(player.last_sequence, None);
    }

    #[test]
    fn test_clear_all_players() {
        let mut players = RemotePlayers::new();

        players.apply(joined(1));
        players.apply(joined(2));

        players.clear();

        assert!(players.is_empty());
    }

    #[test]
    fn test_non_player_messages_are_ignored() {
        let mut players = RemotePlayers::new();

        assert!(!players.apply(ServerMessage::Pong));

        assert!(!players.apply(ServerMessage::Welcome {
            player_id: 1,
            max_players: 3,
        }));

        assert!(!players.apply(ServerMessage::Rejected {
            reason: "Error".to_string(),
        }));

        assert!(players.is_empty());
    }
}
