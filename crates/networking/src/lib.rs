pub mod client;
pub mod protocol;
pub mod server;
pub mod sync;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    PlayerJoined {
        player_id: NetworkPlayerId,
        name: String,
    },
    PlayerMoved {
        player_id: NetworkPlayerId,
        sequence: u64,
        transform: PlayerTransform,
    },
    PlayerLeft {
        player_id: NetworkPlayerId,
    },
}

#[derive(Clone, Default)]
pub struct ServerEventQueue {
    events: Arc<Mutex<VecDeque<ServerEvent>>>,
}

impl ServerEventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pop(&self) -> Option<ServerEvent> {
        self.events.lock().ok()?.pop_front()
    }

    pub(crate) fn push(&self, event: ServerEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push_back(event);
        }
    }
}

pub use client::GameClient;
pub use protocol::{
    decode_client, decode_server, encode, ClientMessage, NetworkPlayerId, ProtocolError,
    ServerMessage, MAX_PLAYERS, PROTOCOL_VERSION,
};
pub use server::{GameServer, GameServerHandle};
pub use sync::{NetworkOrientation, NetworkVec3, PlayerTransform};